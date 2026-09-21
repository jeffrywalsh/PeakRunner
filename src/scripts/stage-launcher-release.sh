#!/bin/sh
# Stage a signed updater feed from a prebuilt external-map client. Does not deploy.
set -eu
cd "$(dirname "$0")/.."
platform=${1:?macos-arm64, windows-x64 or linux-x64}
binary=${2:?absolute path to external-map client binary}
key=${3:?absolute path to private Ed25519 PKCS8 signing key}
sequence=${4:?monotonically increasing release sequence}
notes=${5:?release notes file}
case "$platform" in macos-arm64|windows-x64|linux-x64) ;; *) exit 2 ;; esac
case "$sequence" in ''|*[!0-9]*) exit 2 ;; esac
version=$(sed -n 's/^pub const GAME_VERSION: &str = "\([^"]*\)";.*/\1/p' crates/protocol/src/lib.rs)
stage="local-assets/launcher-releases/$version-$sequence/$platform"
test ! -e "$stage" || { echo "Release stage already exists" >&2; exit 1; }
mkdir -p "$stage/payload/game" "$stage/payload/map"
case "$platform" in
  windows-x64) cp "$binary" "$stage/payload/game/PeakRunner.exe" ;;
  *) cp "$binary" "$stage/payload/game/peakrunner" ;;
esac
cp assets/maps/raindance/* "$stage/payload/map/"
if test "${PEAKRUNNER_PRIVATE_TEST:-}" = 1; then
    python3 scripts/stage-private-maps.py "$stage/payload/game/private-maps"
    # Empty ambience is implicit only when map.json records the empty SHA-256.
    # Existing launcher r1 rejects zero-byte files; all other payloads stay exact.
    python3 - "$stage/payload/game/private-maps" <<'PY'
import hashlib, json, pathlib, sys
for path in pathlib.Path(sys.argv[1]).glob('*/ambient.f32'):
    if path.stat().st_size == 0:
        manifest = json.loads((path.parent / 'map.json').read_text())
        assert manifest['files']['ambient.f32'] == hashlib.sha256(b'').hexdigest()
        path.unlink()
PY
fi
cargo run -p peakrunner-launcher --bin launcher-release -- publish "$key" "$platform" "$version" "$sequence" "$stage/payload" "$notes" "$stage/feed"
echo "Staged $stage/feed; upload blobs before the platform manifest. No deployment performed."
