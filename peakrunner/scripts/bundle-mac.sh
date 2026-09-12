#!/bin/sh
# Build a double-clickable PeakRunner.app from the release binary.
set -eu
cd "$(dirname "$0")/.."

cargo build --release

APP="PeakRunner.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/peakrunner "$APP/Contents/MacOS/peakrunner"
cp assets/Info.plist "$APP/Contents/Info.plist"
chmod +x "$APP/Contents/MacOS/peakrunner"

if [ -f assets/AppIcon.icns ]; then
  cp assets/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
fi

echo "Built $(pwd)/$APP"
