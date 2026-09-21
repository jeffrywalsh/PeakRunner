#!/bin/sh
# PRIVATE local source-game reference, never a downloadable release artifact.
set -eu
cd "$(dirname "$0")/.."
pack=local-assets/broadside-reference
app=PeakRunner-Broadside-Reference.app
launcher=scripts/broadside-reference-launcher.sh
display='PeakRunner Broadside Reference'
identifier=dev.peakrunner.broadside-reference
case "${1:-}" in
    '') ;;
    --workshop)
        pack=local-assets/broadside-workshop
        app=PeakRunner-Broadside-Workshop.app
        launcher=scripts/broadside-workshop-launcher.sh
        display='PeakRunner Broadside Workshop'
        identifier=dev.peakrunner.broadside-workshop
        ;;
    *) echo 'Usage: bundle-broadside-reference.sh [--workshop]' >&2; exit 2 ;;
esac
test -f "$pack/map.json"
test -x target/release/peakrunner
if test -e "$app"; then
    echo "Refusing to overwrite existing $app" >&2
    exit 1
fi
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/map"
cp target/release/peakrunner "$app/Contents/MacOS/peakrunner-bin"
cp "$launcher" "$app/Contents/MacOS/peakrunner"
chmod +x "$app/Contents/MacOS/peakrunner"
cp assets/Info.plist "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleName $display" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName $display" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $identifier" "$app/Contents/Info.plist"
for file in map.json vertices.bin collision.bin height.bin weights.rgba textures.rgba ambient.f32; do
    cp "$pack/$file" "$app/Contents/Resources/map/$file"
done
codesign --force --sign - --timestamp=none "$app/Contents/MacOS/peakrunner-bin"
sh scripts/sign-mac-app.sh "$app"
echo "Created private $app — do not redistribute."
