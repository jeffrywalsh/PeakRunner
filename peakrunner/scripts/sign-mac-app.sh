#!/bin/sh
# Seal the finished bundle, not just its executable. No Developer ID is implied.
set -eu
app=${1:?path to completed .app bundle}
test -f "$app/Contents/Info.plist"
test -d "$app/Contents/MacOS"
codesign --force --sign - --timestamp=none "$app"
codesign --verify --deep --strict --verbose=2 "$app"
