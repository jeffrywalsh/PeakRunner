#!/bin/sh
# Original-asset playtest: never replaces the normal or extracted-reference app.
set -eu
cd "$(dirname "$0")/.."
pack="${1:-assets/maps/raindance}"
app=PeakRunner-Original.app
test -f "$pack/map.json"
test -x target/release/peakrunner
if test -e "$app"; then
    echo "Refusing to overwrite existing $app" >&2
    exit 1
fi
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/map"
cp target/release/peakrunner "$app/Contents/MacOS/peakrunner-bin"
cp scripts/raindance-launcher.sh "$app/Contents/MacOS/peakrunner"
chmod +x "$app/Contents/MacOS/peakrunner"
cp assets/Info.plist "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleName PeakRunner Original' "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleDisplayName PeakRunner Original' "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleIdentifier dev.peakrunner.original-playtest' "$app/Contents/Info.plist"
for file in map.json vertices.bin collision.bin height.bin weights.rgba textures.rgba ambient.f32 shade.rg props.bin; do
    # props.bin (instanced scenery) is optional; the manifest says whether it is needed.
    [ "$file" = props.bin ] && [ ! -f "$pack/$file" ] && continue
    cp "$pack/$file" "$app/Contents/Resources/map/$file"
done
codesign --force --sign - --timestamp=none "$app/Contents/MacOS/peakrunner-bin"
sh scripts/sign-mac-app.sh "$app"
echo "Created $app using the selected map pack."
