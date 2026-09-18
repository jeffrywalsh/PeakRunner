# PeakRunner

Ski and disc capture-the-flag. One Rust crate, three targets: macOS, Windows, and WebAssembly. The match rules and the simulation are the same code on every target.

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

Three processes. The directory only lists games. A server advertises itself there. The game asks the directory who is hosting, then connects to that server.

```sh
cargo run -p peakrunner-net --bin peakrunner-directory
cargo run -p peakrunner-net --bin peakrunner-server -- --name "North Spine"
cargo run --bin peakrunner
```

On the menu, Valley is the small rift. Raindance is the 2 km Tribes terrain, with the ravine between the bases. Choose it, then Start match.

In the game, choose Find match. The directory and a match server both run on dellcon. The directory is `192.168.1.64:7780`. The match server advertises itself there and accepts players on `192.168.1.64:7781`.

## Controls

WASD moves and steers. The mouse looks. Hold Space to ski: a fresh press can hop at low speed, but holding through landing stays smooth. Downhill assistance builds speed, and ramps launch you without pulling you back onto the ground. Right click jets straight up. WASD while jetting steers without spending that climb; additional horizontal jet speed tapers off around 72 km/h. Without jets, WASD still gives gentle air control. Coasting preserves momentum, including while carrying a flag. Left click fires. 1 and 2 switch weapons. Esc pauses.

A gamepad uses the left stick to move, the right stick to look, the south button or left trigger to jump, and the right trigger to fire.

## Movement and visual reference

The target is **Ascend-inspired movement with a Tribes 1-inspired disc launcher**,
not an exact reproduction of either engine. The original launcher reference is
[this Tribes 1 gameplay capture](https://www.mobygames.com/game/2661/starsiege-tribes/screenshots/windows/300912/):
silver split rails, dark feed channel, cyan insets and yellow warning details.
The new model is original procedural geometry, not an imported game asset.
Raindance uses grass and exposed rock; Valley retains its snow treatment.

Current tuning uses 20 m/s² base gravity, 2× downhill assistance, a gradual
gravity reduction between 250 and 350 km/h, a roughly 3.8-second usable jet burn,
and a five-second full recharge. These are deliberate tuning choices for this
game, not claims of exact Ascend constants. Steering preserves speed at high
velocity; the 450 km/h safety ceiling is above normal route speeds. Discs fly
straight at 95 m/s plus 75% of shooter velocity, retaining the existing combat
balance. Ski collision remains a heightfield approximation, not the original
Tribes collision engine.

Validation: `cargo test --lib`, `cargo check --target wasm32-unknown-unknown`,
and `cargo build --release`. The optional GPU test
`cargo test --lib render_gameplay_captures -- --ignored --nocapture`
renders the actual scene at desktop and portrait sizes plus a firing frame into
`screenshots/`. It requires a graphics adapter and does not verify window/input
integration or the HUD. Set `PEAKRUNNER_CAPTURE_LABEL` to distinguish captures.
