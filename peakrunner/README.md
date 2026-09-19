# PeakRunner

Ski and disc capture-the-flag. A shared Rust simulation, a native/WASM client,
and a headless match server. Online matches currently use the native client;
the browser build retains offline play.

## Play on this Mac

```sh
cargo run --release
```

A double-clickable app:

```sh
./scripts/bundle-mac.sh
open PeakRunner.app
```

## Windows

On a Windows machine, from this directory:

```sh
cargo build --release
```

That produces `target\release\peakrunner.exe`. Release builds do not open a console window.

From a Mac with the `x86_64-pc-windows-gnu` target and a mingw linker, the same crate cross-compiles:

```sh
cargo build --release --target x86_64-pc-windows-gnu
```

The linker is set in `.cargo/config.toml`.

## WebAssembly

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve
```

`./scripts/build-wasm.sh` does the release bundle when `trunk` is installed, and otherwise just checks that the wasm target compiles.

## Multiplayer

Eight-player, server-authoritative CTF with encrypted public matches. The server
runs movement, projectiles, damage, energy, respawns, flags, scores, and the match
clock at 60 Hz. Clients predict their movement and reconcile to 20 Hz snapshots.
The default directory is `https://dir.peakrunner.net/servers`; direct encrypted
joining uses `quic://play.peakrunner.net:7777`. The VPS match has a public CA
certificate and passed initial eight-client WAN checks. Choose **Find match** in the updated
desktop client. The directory is optional; direct joining works without it.

```sh
cargo run --release -p peakrunner-net --bin peakrunner-server -- --name "North Spine" --map Valley
cargo run --bin peakrunner
```

Choose **Find match → Join directly** for the local server at `127.0.0.1:7781`.
Or use **Find match → Host a private match → Host and join** to host from the
game itself; closing your hosted match disconnects its guests.
For friends, bind the server to its specific LAN/VPN IP with `--bind ADDRESS`,
and use that address in their native clients. The server selects Valley or
Raindance for everyone. Teams are balanced on join; two opposing players start
a three-second countdown. First to three captures or eight minutes ends the
round; the next round starts after ten seconds. Tab shows the roster and K/D.
Esc opens a menu without pausing the server or protecting your player.

Discovery uses certificate-verified HTTPS through Cloudflare Tunnel on dellcon.
Gameplay uses certificate-verified QUIC datagrams directly to the VPS over UDP;
Cloudflare does not relay gameplay. The server operator remains trusted.
Legacy direct IP connections still use plaintext TCP: use only on a trusted LAN
or VPN, never port-forward them. Optional match passwords are not verified player
accounts. See [multiplayer hosting and limits](docs/multiplayer.md)
for directory setup, tests, security boundaries, and remaining work.

## Controls

WASD moves and steers. The mouse looks. Hold Space to ski: a fresh press can hop at low speed, but holding through landing stays smooth. Downhill assistance builds speed, and ramps launch you without pulling you back onto the ground. Right click jets straight up. WASD while jetting steers without spending that climb; additional horizontal jet speed tapers off around 72 km/h. Without jets, WASD still gives gentle air control. Coasting preserves momentum, including while carrying a flag. Left click fires. 1, 2, and 3 select Disc, Chaingun, and Grenade Launcher. Esc pauses.

The chaingun fires small, fast physical bullets with slight spread and short amber tracers: direct-hit damage only, no splash or explosion on expiry. Grenades leave a fading smoke trail and bounce during a 0.35-second arming delay, then detonate on contact with terrain, pillars, or enemies. A two-second fuse remains as a fallback. These are original Tribes-inspired weapons, not exact replicas of the original balance. Controller/touch weapon-swap cycles all three slots.

A gamepad uses the left stick to move, the right stick to look, the south button or left trigger to jump, and the right trigger to fire.

## Movement and visual reference

The target is **Ascend-inspired movement with a Tribes 1-inspired disc launcher**,
not an exact reproduction of either engine. The original launcher reference is
[this Tribes 1 gameplay capture](https://www.mobygames.com/game/2661/starsiege-tribes/screenshots/windows/300912/):
silver split rails, dark feed channel, cyan insets and yellow warning details.
The new model is original procedural geometry, not an imported game asset.
Raindance uses gritty ground-only turf, soil, and exposed rock—no grass blades.
World-fixed detail tiles and per-map material settings separate surface grain
from broad color patches; Valley retains its snow treatment. See the
[terrain art direction and research](docs/terrain-art-direction.md) for the
historical references and next-landscape pipeline.

Current tuning uses 20 m/s² base gravity, 2× downhill assistance, a gradual
gravity reduction between 250 and 350 km/h, a roughly 3.8-second usable jet burn,
and a five-second full recharge. These are deliberate tuning choices for this
game, not claims of exact Ascend constants. Steering preserves speed at high
velocity; the 450 km/h safety ceiling is above normal route speeds. Discs fly
straight at 95 m/s plus 75% of shooter velocity, retaining their existing speed
and inheritance. Movement tuning is frozen after playtesting.
Ground collision now samples the same triangle faces drawn on both maps, with
swept airborne contacts resolved before rendering. Supported skiing retains its
smooth normal response to avoid adding friction at triangle seams.

Discs use swept terrain, pillar and player checks, resolving the first hit along
their path. Terrain and midfield pillars shield splash; damage tapers to zero
at the blast edge. The existing 7.5 m radius, 1.05 s reload and disc-jump impulse
are retained. Shots originate on the eye-height aim line and gun recoil is
visual, so repeated disc fire does not walk the aim upward. The launcher has a
mechanical feed animation, a tapered flight trail and shared native/web
synthesized firing and ready cues. Decorative base structures still need their
own collider pass when the bases are rebuilt.

Validation: `cargo test --lib`, `cargo check --target wasm32-unknown-unknown`,
and `cargo build --release`. The optional GPU test
`cargo test --lib render_gameplay_captures -- --ignored --nocapture`
renders the actual scene at desktop and portrait sizes, firing/reload stages,
and a staged projectile view into `screenshots/`. It requires a graphics
adapter and does not verify window/input
integration or the HUD. Set `PEAKRUNNER_CAPTURE_LABEL` to distinguish captures.
