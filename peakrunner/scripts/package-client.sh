#!/bin/sh
# Package a prebuilt client without overwriting previous releases.
set -eu
cd "$(dirname "$0")/.."
platform=${1:?macos-arm64, windows-x64 or linux-x64}
binary=${2:?absolute path to compiled client}
version=$(sed -n 's/^pub const GAME_VERSION: &str = "\([^"]*\)";.*/\1/p' crates/protocol/src/lib.rs)
revision=${3:-}
case "$revision" in *[!a-zA-Z0-9-]*) echo "Invalid package revision" >&2; exit 2 ;; esac
package_version="$version${revision:+-$revision}"
case "$platform" in macos-arm64|windows-x64|linux-x64) ;; *) exit 2 ;; esac
test -f "$binary"
release_dir="local-assets/releases/$package_version/$platform"
test ! -e "$release_dir" || { echo "Release already exists: $release_dir" >&2; exit 1; }
mkdir -p "$release_dir"
cp docs/client-release-notes.md "$release_dir/README.md"
case "$platform" in
  macos-arm64)
    app="$release_dir/PeakRunner.app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$binary" "$app/Contents/MacOS/peakrunner"
    cp assets/Info.plist "$app/Contents/Info.plist"
    if test -f assets/AppIcon.icns; then cp assets/AppIcon.icns "$app/Contents/Resources/"; fi
    chmod +x "$app/Contents/MacOS/peakrunner"
    sh scripts/sign-mac-app.sh "$app"
    ;;
  windows-x64) cp "$binary" "$release_dir/PeakRunner.exe" ;;
  linux-x64) cp "$binary" "$release_dir/peakrunner"; chmod +x "$release_dir/peakrunner" ;;
esac
archive="PeakRunner-$package_version-$platform"
case "$platform" in
  linux-x64) tar -czf "$release_dir/../$archive.tar.gz" -C "$release_dir" . ;;
  *) (cd "$release_dir" && zip -qr "../$archive.zip" .) ;;
esac
echo "Packaged $release_dir"
