# Rebuild, restore and operate

The public game server is independent of dellcon. Keep the website and HTTPS
directory on dellcon; do not proxy the game UDP endpoint through its tunnel.

## Recreate the image

`source.json` identifies the source revision and Linux/amd64 image tag. In a
checkout of that revision, build from the `peakrunner` directory:

```sh
docker build --platform linux/amd64 -f crates/net/Dockerfile -t peakrunner/public:8d4437e .
docker save peakrunner/public:8d4437e | gzip > peakrunner-server-image.tar.gz
```

The Dockerfile pins both base image digests and Cargo uses the committed lockfile.
Transfer the image over SSH and load it with `docker load`. An exact saved image
preserves the ID in `release.conf`; a fresh rebuild can have different attestation
metadata. Inspect its image ID, record it in `release.conf`, and test before use.
No container registry account is required. Git stores source/config, not images.

## Recreate the VPS

1. Install Docker/Compose, Certbot and its Cloudflare DNS plugin on supported
   Ubuntu. Set key-only SSH and permit TCP 22 plus UDP 7777. Test a second SSH
   session before closing the first. Do not publish the internal HTTP endpoint.
2. Copy this directory (without secrets) to `/opt/peakrunner`. Restore the
   zone-scoped credential to `/etc/letsencrypt/cloudflare.ini`, root-only mode
   600, from a password manager/encrypted backup. Alternatively run
   `provision-certificate.mjs` on the administrator machine with
   `CF_API_TOKEN_FILE` pointing at the broad local token. Its current host is
   explicitly pinned; review that destination for a move. It refuses to mint a
   duplicate named token if an existing credential needs recovery.
3. Obtain a DNS-01 certificate for `play.peakrunner.net`; install
   `renew-certificate.sh` root-owned mode 755 as
   `/etc/letsencrypt/renewal-hooks/deploy/peakrunner`. Run it initially with
   `RENEWED_LINEAGE=/etc/letsencrypt/live/play.peakrunner.net` before creating
   `release.conf` on a fresh host. The hook installs keys mode 440, root:10001.
4. Load the image, copy its reviewed `release.conf`, then run:

   ```sh
   cd /opt/peakrunner
   docker compose --env-file release.conf up -d
   docker compose --env-file release.conf ps
   docker compose --env-file release.conf logs --tail 30
   certbot renew --dry-run --cert-name play.peakrunner.net
   ```

5. Test UDP by IP override while still verifying the public hostname certificate.
   Only after passing, change the DNS-only gameplay A record. The guarded
   Cloudflare helper lives in `../dellcon/cloudflare.mjs`; review its JSON target
   before moving hosts. Existing unexpected DNS records are preserved/refused.

## Routine checks and limitations

- Docker health checks require an advancing simulation clock. Compose restarts
  exited processes, **not** merely unhealthy ones; investigate unhealthy status.
- `certbot.timer` handles renewal; the deploy hook restarts the game to load its
  new certificate. This briefly disconnects active players. Schedule maintenance
  and warn players for organized matches; seamless certificate reload is not built.
- The default public match has eight slots and no password. Display names are
  not authenticated accounts. Encryption does not prevent aim assistance or DDoS.
- Run `public_smoke` from an outside machine for transport checks, but use human
  multi-machine playtests to judge prediction, fairness and match feel.
- `udp-test-proxy.mjs` is a localhost-only diagnostic, never a production relay.
- Keep private keys, renewal credentials, passwords and recovery codes in a
  separate encrypted backup. Never commit them or print full container metadata.
- Roll back only to a recorded compatible protocol release. Use maintenance
  routing for an unavailable game rather than sending raw UDP to Cloudflare Tunnel.
