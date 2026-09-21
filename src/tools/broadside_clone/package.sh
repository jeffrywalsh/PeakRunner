#!/bin/sh
# New private packaging path. No prior map compiler or bundle helper is invoked.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
reference="$root/../research/apps/PeakRunner-Broadside-Reference.app"
output="$root/../research/apps/${2:-PeakRunner-Broadside-Clone.app}"
pack="$root/local-assets/broadside-clone/${1:-compiled-v1}"
test ! -e "$output" || { echo 'Refusing to overwrite clone app'; exit 1; }
test -f "$pack/map.json"
/usr/bin/codesign --verify --deep --strict "$reference"
/usr/bin/ditto "$reference" "$output"
# Replace the map only with fresh scene-compiler output, never a generated fortress.
for file in vertices.bin collision.bin height.bin weights.rgba textures.rgba ambient.f32 map.json; do
    cp "$pack/$file" "$output/Contents/Resources/map/$file"
done
/usr/bin/plutil -replace CFBundleName -string 'Broadside Clone' "$output/Contents/Info.plist"
/usr/bin/plutil -replace CFBundleDisplayName -string 'Broadside Clone' "$output/Contents/Info.plist"
/usr/bin/plutil -replace CFBundleIdentifier -string 'dev.peakrunner.broadside-clone' "$output/Contents/Info.plist"
/usr/bin/codesign --force --sign - "$output"
/usr/bin/codesign --verify --deep --strict "$output"
cmp "$reference/Contents/MacOS/peakrunner-bin" "$output/Contents/MacOS/peakrunner-bin"
for file in collision.bin height.bin weights.rgba ambient.f32; do
    cmp "$reference/Contents/Resources/map/$file" "$output/Contents/Resources/map/$file"
done
if test "$#" -eq 0; then
    for file in vertices.bin textures.rgba; do
        cmp "$reference/Contents/Resources/map/$file" "$output/Contents/Resources/map/$file"
    done
fi
echo 'Clone packaged: approved executable and collision/terrain/audio match exactly.'
