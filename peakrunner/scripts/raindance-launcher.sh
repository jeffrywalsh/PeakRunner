#!/bin/sh
set -eu
binary_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export PEAKRUNNER_MAP_PACK="$binary_dir/../Resources/map"
exec "$binary_dir/peakrunner-bin" "$@"
