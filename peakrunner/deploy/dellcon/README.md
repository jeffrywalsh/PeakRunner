# PeakRunner public infrastructure

This directory is the portable deployment unit. Its production copy lives at
`/data/peakrunner/public` in dellcon's homelab repository. Keep both copies in
sync when changing infrastructure. Docker Compose project: `peakrunner-public`.

## What is deployed

- A digest-pinned cloudflared container, `peakrunner-cloudflared`, with automatic
  restart, bounded logs/resources, read-only filesystem and dropped capabilities.
- Dedicated external Docker network `peakrunner-public`, created by the helper.
- Three proxied DNS names and explicit tunnel ingress in `cloudflare.json`.
- HTTPS directory at `dir.peakrunner.net/servers`, polling the VPS over validated
  QUIC. Only operator-configured official matches are listed.
- `play.peakrunner.net` is a DNS-only A record to the VPS, serving UDP 7777.
  It does not send gameplay through Cloudflare Tunnel.
- Apex/www still return HTTP 503 maintenance; the entry page is separate work.
  Old LAN-only game containers remain managed by `/data/docker-compose.yml`.

There are no published inbound ports for these dellcon services. Cloudflare routes directly to this dedicated tunnel;
existing Caddy configuration is unchanged and remains in the homelab repository.
Do not point these HTTP routes at the legacy raw TCP game ports.

## Restore or move to another machine

1. Clone the homelab repository onto the replacement Docker host, preserving this
   directory. Install Docker Engine with Compose v2+ and verify its SSH host key.
   Alternatively copy this entire directory from the PeakRunner repository.
   Rebuild or load the directory-only image using this directory's `source.json`.
   From that revision's `peakrunner` workspace, build with
   `docker build -f crates/directory/Dockerfile -t peakrunner/directory:local .`.
   Inspect its ID and update the directory service's image pin before deploying.
   `source/` is an ignored build context, not a source backup. The VPS server has
   its own image and source record in `../vps/`; compatible protocols are required,
   but directory and server updates can be deployed independently.
2. On an administrator machine with Node 22+, recover the Cloudflare API token
   from your password manager/encrypted backup into an owner-only file **outside
   Git**. Required access: account Tunnel Edit, peakrunner.net DNS Edit and Zone
   Read. The account/zone must already exist; registrar/nameserver migration is
   not performed by this script.
3. Set `CF_API_TOKEN_FILE` to that file, `PEAKRUNNER_SSH_HOST` to the replacement
   SSH destination, and `PEAKRUNNER_DEPLOY_DIR` to the absolute path of this
   directory on that host. Defaults target dellcon's existing deployment.
4. Run `node cloudflare.mjs --apply --start` from the admin copy. It reuses the
   named tunnel, creates missing DNS and network resources, obtains the
   tunnel-scoped credential and starts Compose remotely. API secrets are neither
   stored in this directory nor printed. Conflicting DNS is never overwritten.
   Use `--apply --sync-config` to apply the reviewed ingress configuration.
5. Verify the tunnel is healthy (`node cloudflare.mjs`), `/servers` returns the
   live VPS listing, and apex/www return maintenance. Gameplay uses QUIC, not an
   HTTPS page. Check each connector when both hosts are active.
6. After verifying the replacement, stop the old connector. Both machines can
   serve the same tunnel during migration; deploy matching backend services on
   both before serving real traffic. No DNS change is required when reusing the
   same tunnel. Do not delete the tunnel during a normal host move.

If the Cloudflare tunnel itself was deleted, the helper creates a replacement but
refuses stale DNS conflicts. Review and explicitly replace only the four stale
PeakRunner CNAME targets; it will not silently repoint existing records.
The gameplay A record is independent of tunnel recreation.

## Routing changes and upgrades

Edit `cloudflare.json`, review/commit it, then run
`node cloudflare.mjs --apply --sync-config`. This explicitly replaces this
tunnel's routing configuration; ordinary `--apply` preserves existing routing.
No flags means read-only status; `--start` only starts/reconciles Compose.
`--apply --cutover-udp` changes only the expected gameplay tunnel CNAME into
the configured DNS-only A record, or creates it if absent. Run it only after
certificate-verified WAN checks. Unexpected existing DNS is refused/preserved.

Pin a reviewed cloudflared image digest in `compose.yaml`, commit it, copy the
updated deployment to the host, then run the helper with `--start`. Restore the
previous Compose revision and run `--start` to roll back an image change.

Never print full `docker inspect` or expanded `docker compose config` output:
the tunnel credential resides in container environment metadata, readable by
Docker administrators. The broad API token stays on the admin machine. Docker
restart/reboot needs no token file; container recreation retrieves it from
Cloudflare. Back up account recovery codes/API credentials separately in your
password manager or encrypted secret backup. Git is not a credential backup.

## Initial migration rollback

The original standalone container is `peakrunner-tunnel`. While retained in a
stopped state it can be restarted for immediate rollback after stopping
`peakrunner-cloudflared`. It is not part of the desired deployment and should be
removed only after verifying the Compose deployment. Do not remove unrelated
homelab containers, networks or volumes.
