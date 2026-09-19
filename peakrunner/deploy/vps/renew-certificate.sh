#!/bin/sh
# Certbot deploy hook; installed root-owned and executable on the VPS.
set -eu
test "${RENEWED_LINEAGE:-}" = /etc/letsencrypt/live/play.peakrunner.net || exit 0
install -d -m 750 -o root -g 10001 /opt/peakrunner/secrets /opt/peakrunner/secrets/certs
install -m 440 -o root -g 10001 "$RENEWED_LINEAGE/fullchain.pem" /opt/peakrunner/secrets/certs/fullchain.pem
install -m 440 -o root -g 10001 "$RENEWED_LINEAGE/privkey.pem" /opt/peakrunner/secrets/certs/privkey.pem
# TLS configuration is loaded at startup. Renewal briefly disconnects players;
# clients can reconnect. Run renewal in the maintenance window where possible.
if test -f /opt/peakrunner/release.conf; then
  cd /opt/peakrunner
  docker compose --env-file release.conf -f compose.yaml restart match
fi
