#!/bin/sh
set -eu
binary_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export PEAKRUNNER_MAP_PACK="$binary_dir/../Resources/map"
export PEAKRUNNER_CONFIG_DIR="$HOME/Library/Application Support/PeakRunner/BroadsideReference"
unset PEAKRUNNER_JOIN PEAKRUNNER_MATCH_PASSWORD
exec "$binary_dir/peakrunner-bin" "$@"
