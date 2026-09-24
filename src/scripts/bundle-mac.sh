#!/bin/sh
# Build a double-clickable PeakRunner.app from the release binary.
set -eu
cd "$(dirname "$0")/.."

cargo build --release

APP="PeakRunner.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/map"
cp target/release/peakrunner "$APP/Contents/MacOS/peakrunner"
cp assets/Info.plist "$APP/Contents/Info.plist"
chmod +x "$APP/Contents/MacOS/peakrunner"
for file in map.json vertices.bin collision.bin height.bin weights.rgba textures.rgba ambient.f32 shade.rg props.bin; do
  # props.bin (instanced scenery) is optional; the manifest says whether it is needed.
  [ "$file" = props.bin ] && [ ! -f "assets/maps/raindance/$file" ] && continue
  cp "assets/maps/raindance/$file" "$APP/Contents/Resources/map/$file"
done

if [ -f assets/AppIcon.icns ]; then
  cp assets/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
fi

sh scripts/sign-mac-app.sh "$APP"
echo "Built $(pwd)/$APP"
