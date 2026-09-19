# PeakRunner public hosting

## Infrastructure bootstrap

The `peakrunner-dellcon` remotely managed Cloudflare Tunnel is dedicated to
PeakRunner. The intended public names are `peakrunner.net`, `www.peakrunner.net`,
`dir.peakrunner.net`, and `play.peakrunner.net`. Proxied CNAME records target the
tunnel, not dellcon's private address. No wildcard hostname or private network
route is configured.

The bootstrap initially returns HTTP 503 for these four names and 404 for
everything else. **This is network provisioning, not a playable deployment.**
The landing page, HTTPS directory, and secure WebSocket client/server transport
still need implementation and deployment before replacing the maintenance rules.
Do not route HTTP to the existing raw TCP services on 7780/7781.

The portable deployment is in `deploy/dellcon/`, mirrored to
`/data/peakrunner/public` in the homelab repository. Its README covers restoration,
credentials, routing updates and rollback. `scripts/cloudflare-bootstrap.mjs`
delegates to that helper and defaults to read-only status. `--apply` creates
missing Cloudflare resources; `--start` reconciles Docker Compose. Explicit
`--apply --sync-config` applies the version-controlled ingress configuration.

## Dellcon

- SSH: `jeffryw@192.168.1.64`.
- Compose project: `peakrunner-public`; container: `peakrunner-cloudflared`,
  restart policy `unless-stopped`.
- Dedicated Docker network: `peakrunner-public`.
- No published ports, Docker socket mount, or host-network access.
- Read-only filesystem, dropped capabilities, no-new-privileges, bounded logs,
  memory and CPU.
- Existing `/data/docker-compose.yml`, Caddy sites and legacy PeakRunner services
  are not modified. The tunnel uses its own version-controlled Compose project
  under `/data/peakrunner/public`.
- The container image is pinned by digest in `deploy/dellcon/compose.yaml`.

## Credentials

The local `cf-key` file is Git-ignored and owner-readable/writable only. It may
contain a plain API token or a Bearer authorization example; its contents must
never be printed, sourced as shell commands, or committed.

The broad Cloudflare account token remains local. Only the tunnel-scoped token
is delivered to dellcon over SSH stdin. It is stored in the container environment
and is accessible to Docker administrators. Do not publish full container inspect
output. Recreating the container requires retrieval of the tunnel token from
Cloudflare. Prefer a dedicated limited API token (Tunnel Edit for the account,
DNS Edit and Zone Read for peakrunner.net) over an all-permissions token.

## Release checks still required

1. Deploy the entry page and HTTPS directory without exposing other homelab apps.
2. Add WSS transport and test authoritative matches through Cloudflare, including
   disconnects, message limits, abuse limits, and slow consumers.
3. Connect those services to the dedicated network and update explicit ingress
   routes; redirect www to the apex site.
4. Verify HTTPS from outside the LAN and run a real multi-client WAN match.
5. Keep reconnect handling: Cloudflare can terminate long-lived WebSockets during
   infrastructure updates. Cloudflare WAF does not inspect gameplay messages after
   the WebSocket upgrade; server validation remains necessary.

References:
- https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/get-started/create-remote-tunnel-api/
- https://developers.cloudflare.com/network/websockets/
