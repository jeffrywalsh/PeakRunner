# Three-directory workspace

Active repository: `~/workspace/peakrunner`. Build root: its `src/` directory.
On this case-insensitive volume the old name collided with the destination.
The outer repository was renamed to `PeakRunner.bak`, then its native project
and Git metadata were APFS-cloned into the new checkout. Uncommitted and private
data remain recoverable in the intact backup. No backup deletion is authorized.
The old browser scaffold is retained there and in history, not in the active tree.

| Directory | Contents |
|---|---|
| `src/` | Complete build workspace: client, server, directory, launcher, site, tools |
| `docs/` | Guides, runbooks and historical validation notes |
| `research/` | Ignored private assets, screenshots, app bundles and secrets |

Required original map data, icons/plists, manifests/lockfiles, cross-target
configuration and Docker recipes are build inputs and stay in `src/`, even
though some are not code. Root retains README/AGENTS/ignore and Git metadata.
The local build cache is ignored under `src/target`.

## Build from src

```sh
cd ~/workspace/peakrunner/src
cargo build --locked --release -p peakrunner --bin peakrunner
cargo build --locked --release -p peakrunner-server -p peakrunner-directory -p peakrunner-launcher
cargo build --locked --release --target x86_64-pc-windows-gnu -p peakrunner --bin peakrunner
cargo check --locked --target wasm32-unknown-unknown -p peakrunner --lib
docker build -f deploy/client-linux.Dockerfile -t peakrunner/client-build:local .
```

Windows cross-build needs MinGW-w64 and the Rust target; macOS needs the Xcode
command-line tools. Linux Docker needs Docker and base-image/package access.
Compilers, SDKs and Cargo dependencies are not vendored. Cross-build success
does not certify native Windows/Linux gameplay.

Linux client/launcher and match-server Docker recipes now copy both embedded
original packs (Raindance and Skybreak), fixing the older missing Skybreak input.
Docker context is `src/`, not repository root. No live service is changed.

## Optional local compatibility

Ignored links `src/local-assets -> ../research/local-assets`,
`src/screenshots -> ../research/screenshots`, `src/docs -> ../docs` keep historical
research commands working locally. Normal binary compilation needs none of them.
Docker ignores them. Imported maps remain optional private local installs.
Packaging carries README inputs under `assets/package-docs/`, so it does not
depend on the external docs link. Keep those copies synchronized with the guides.

Reference bundles moved to `research/apps/`; their frozen launch scripts may
contain old paths. New work should use the current client, not those bundles.

Git history, remotes and stashes were retained; the detached review-worktree
linkage was repaired. The requested main commit is local, not a push/deployment.
Use `PeakRunner.bak` or Git history for recovery; do not delete the backup yet.

## Migration validation

- Relocated workspace all-targets check and full library tests passed.
- macOS release client and Windows x64 release client built successfully.
- WebAssembly library check passed (existing unused Lobby warning).
- A standalone copy of Cargo inputs, with no docs/research links, passed
  an offline all-targets workspace check using the installed dependency cache.
- App dependency boundaries, website JavaScript checks and 19 map-tool tests pass.
- The real native client selected Snowblind and started a match after migration.
  Private map installs still resolve.
- Linux amd64 release client, launch-smoke example and networking-smoke example
  built on dellcon on 2026-09-21 from commit `a4f00b5`, with user-approved
  transfer of Cargo source and only the original Raindance/Skybreak assets.
  Image: `peakrunner-layout-validation:20260921`, immutable ID
  `sha256:2d2edf1e7bc678ec57c58f4d02bd05d21cdf6ec5d87c8ddb23673100a241a35f`.
- Linux runtime smoke passed (exit 0) with Xvfb/Mesa software Vulkan and
  `QA_LOCAL=1`: the real client joined an in-container match and captured a
  frame. Visually inspected terrain, weapon and HUD, including warmup state.
  Capture: `research/screenshots/peakrunner-linux-layout.png` (ignored).
  QA container used `--init --network none`, no host mounts/ports and isolated
  preferences. No private map assets or credentials were transferred.
- Headless runtime emitted missing XDG runtime-directory and ALSA-device
  warnings. Audio playback, physical GPU performance and sustained multiplayer
  gameplay remain untested by this check. The networking-smoke utility was
  compiled, not run against any public server. No deployment/restart occurred.
- Staging audit includes only `research/README.md`; private credentials, reference
  map packs, captures, app bundles and build caches are excluded.
