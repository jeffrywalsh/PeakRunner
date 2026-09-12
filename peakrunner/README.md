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

## Controls

WASD moves. The mouse looks. Space jumps; hold it through a landing to ski, and hold it in the air to jet. Click fires. 1 and 2 switch weapons. Esc pauses.

A gamepad uses the left stick to move, the right stick to look, the south button or left trigger to jump, and the right trigger to fire.
