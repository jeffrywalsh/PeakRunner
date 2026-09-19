# Native client builds

Client, match server and directory remain separate applications. The directory
does not need rebuilding for name/chat protocol changes. Deploy the matching
match server and client downloads together; old gameplay protocols are rejected.

## Build and package

On Apple Silicon:

```sh
cargo build --locked --release -p peakrunner --bin peakrunner
sh scripts/package-client.sh macos-arm64 "$PWD/target/release/peakrunner"
```

Windows x86-64 cross-build from macOS uses the installed MinGW-w64 linker in
`.cargo/config.toml` and the `x86_64-pc-windows-gnu` Rust target:

```sh
cargo build --locked --release --target x86_64-pc-windows-gnu -p peakrunner --bin peakrunner
sh scripts/package-client.sh windows-x64 "$PWD/target/x86_64-pc-windows-gnu/release/peakrunner.exe"
```

Linux x86-64 builds on dellcon using the Debian 12 recipe:

```sh
docker build -f deploy/client-linux.Dockerfile -t peakrunner/client-build:local .
```

Extract `/src/target/release/peakrunner` from the resulting image, then pass its
absolute path to `sh scripts/package-client.sh linux-x64`. The packaging script
refuses to overwrite release directories, embeds no credentials and does not
publish anything. Packages live under `local-assets/releases/<GAME_VERSION>/`.

## Verification

- `cargo test --workspace --lib` runs shared policy, chat-input, team privacy,
  transport and server tests. Socket tests need local networking permission.
- `cargo check --workspace --all-targets` checks all native targets/examples.
- `cargo check --target wasm32-unknown-unknown -p peakrunner --lib` checks web
  compatibility, but is not a browser release.
- `examples/launch_smoke.rs` delegates both application logic and rendering,
  captures the actual native window, then closes only its own test window.
  Set `QA_CAPTURE_PATH`; optionally `PEAKRUNNER_JOIN`, `QA_PAUSE=1` or
  `QA_CHAT=team` / `QA_CHAT=public` for a local test match.
- Linux launch tests can run under Xvfb and Mesa software Vulkan in an isolated
  container. Use `docker run --init` so Xvfb startup signals reach its wrapper.
  This proves rendering compatibility, not gameplay performance. The current
  Linux build passed this check after adding the X11 keyboard library; sound
  devices were absent, so audio and interactive Linux gameplay remain untested.
- Windows build and DLL inspection are not substitutes for Windows runtime QA.
  Use a Windows machine or VM with working accelerated graphics for release QA.

Mac packaging now ad-hoc signs the completed bundle (including its resource seal)
and verifies it with `codesign --verify --deep --strict`. This fixes the invalid
bundle signature; it does not provide Developer ID signing or Apple notarization.
Verify again after extracting the ZIP. No system-wide Gatekeeper bypass is needed.
An optional third package-script argument creates a new packaging revision without
overwriting a published archive or changing the gameplay version.

The playtest packages are published on peakrunner.net with a
matching server. The packaging commands alone do not change public
downloads or the live server. No VM is needed for compilation; native
Windows testing is still required before claiming Windows runtime support.
