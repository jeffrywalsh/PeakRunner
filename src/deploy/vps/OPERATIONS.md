# Rebuild, restore and operate

The public game server is independent of dellcon. Keep the website and HTTPS
directory on dellcon; do not proxy the game UDP endpoint through its tunnel.

The six-map rotation is Raindance, Skybreak Bastions, and the four reference
layouts. No firewall or password change was requested. See the six-map section
in repository-root `AGENTS.md` and `docs/release-20260921-1.md`. The deployed
image still checks `PEAKRUNNER_PRIVATE_TEST`; current source does not. The
running container mounts the reference packs read-only. Preserve that directory
alongside the recorded image and source archives when restoring. It is not
embedded in the image or backed up by Git. Old `.4` clients do not match the
collection's gameplay protocol. Build commands now run from repository `src/`.

## Administration access

Connect as `peakrunner-admin@198.12.80.145` with the existing SSH key. Direct root,
password and keyboard-interactive SSH logins are disabled. The admin has the
explicitly authorized `NOPASSWD: ALL` rule in `/etc/sudoers.d/peakrunner-admin`
(root:root, 0440); use `sudo -n`. This intentionally grants root-equivalent access.
The account is not a Docker-group member. Existing unrelated accounts are preserved.

Source templates: `peakrunner-admin.sudoers` and `00-peakrunner-hardening.conf`.
On a replacement host, create the admin account, install recovered authorized
public keys with directory mode 0700/file mode 0600 and correct ownership,
validate sudoers with `visudo -cf`, and test a fresh SSH connection plus
`sudo -n id` BEFORE installing the root-login restriction. Keep SSH keys out of
Git. Keep a recovery session and timed rollback until verification succeeds.
Validate with `sshd -t`, reload SSH, verify new admin access and rejection of root
and password-only login attempts, then cancel rollback.

The migration backup of the previous SSH snippet is in
`/root/peakrunner-ssh-migration-20260919/`. Recover through the working admin account
or provider console if necessary; the root account was not deleted or disabled
for console use. The temporary rollback timer was canceled after successful tests.
Upload reviewed configs to a staging directory in the admin home, then use
`sudo -n install` to place them in `/opt/peakrunner`; do not make production config
directories world-writable. `provision-certificate.mjs` uses this admin plus sudo.

## Recreate the image

`source.json` identifies the source revision and Linux/amd64 image tag. In a
checkout of that revision, build from the `peakrunner` directory:

```sh
docker build --platform linux/amd64 -f crates/server/Dockerfile -t peakrunner/server:local .
docker save peakrunner/server:local | gzip > peakrunner-server-image.tar.gz
```

The Dockerfile pins both base image digests and Cargo uses the committed lockfile.
Use the release tag recorded in `source.json` instead of `:local` for a published
build. This image contains only `peakrunner-server`. The directory has its own
Dockerfile, source record, and image in `../dellcon/`; do not deploy this server
image to the directory. The former `peakrunner-quic` command is now
`peakrunner-server` (including `--healthcheck`).
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
   sudo -n docker compose --env-file release.conf up -d
   sudo -n docker compose --env-file release.conf ps
   sudo -n docker compose --env-file release.conf logs --tail 30
   sudo -n certbot renew --dry-run --cert-name play.peakrunner.net
   ```

5. Test UDP by IP override while still verifying the public hostname certificate.
   Only after passing, change the DNS-only gameplay A record. The guarded
   Cloudflare helper lives in `../dellcon/cloudflare.mjs`; review its JSON target
   before moving hosts. Existing unexpected DNS records are preserved/refused.

## Routine checks and limitations

- `PEAKRUNNER_MATCH_NAME` in Compose sets the public display name, currently
  `Springdale Central`. The directory reads it from the live server; no separate
  directory rename is needed. The stable directory ID is retained for clients.

- The public match runs Raindance (2 km). `PEAKRUNNER_MATCH_MAP` in Compose
  selects `Valley` or `Raindance`; invalid names fail startup. Change it only
  when the match is empty, then recreate the container. No client update needed.

- Teams are balanced by player count, not skill. New arrivals join the smaller
  side; departures trigger balancing on the next server tick when the difference
  exceeds one. Transfers prefer non-carriers, then dead players, then newest
  arrivals. A transferred player respawns, retains personal stats, drops any
  carried flag, and loses in-flight projectiles. Scores and match time continue.
- The last departure resets all match state immediately to fresh warmup,
  including round, flags, scores, clock and effects. The server tick continues
  monotonically for health checks. Two opponents trigger a fresh countdown.

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

## Game-port flood filtering (`ddos.nft`)

`ddos.nft` is an nftables table (`inet peakrunner_ddos`) that filters UDP
7777–7790 only. SSH and every other port are untouched. It hooks prerouting
at priority -150: after connection tracking, and before Docker's port
translation. It has to sit there because Docker's published ports are
forwarded to the containers and never pass through the input hook.

**Rules:**
- **Size and fragments:** oversized datagrams (over 1500 bytes) and UDP
  fragments are dropped.
- **New flows:** each source IP may open 20 new flows per minute (burst 10).
  A UDP flow stays "new" until the server replies, and a real server replies
  at once.
- **Packet rate:** each source IP may send 400 packets/s (burst 800). A real
  client sends about 70–130. A source over the limit is blocked for 20 s.
  The block is short because UDP sources can be spoofed to get a real player
  blocked.

**Tested** on dellcon in an isolated privileged container (its own network
namespace) against a replying UDP server:
- a normal client at 120/s delivered 351/351 packets;
- a 3000/s flood delivered 969/4661 before its source was blocked;
- the same IP was then blocked;
- 1600-byte packets were all dropped.

**Limits:** this stops floods that reach the host. A flood that fills the
uplink itself needs the provider's upstream DDoS filtering. It also doesn't
protect the website, which is behind Cloudflare's tunnel and needs no
inbound port.

**Install** (keeps working across reboots; nothing is applied until you run
it):

    sudo -n install -m 644 ddos.nft /opt/peakrunner/ddos.nft
    sudo -n nft -c -f /opt/peakrunner/ddos.nft          # dry run
    sudo -n install -m 644 peakrunner-ddos.service /etc/systemd/system/
    sudo -n systemctl daemon-reload && sudo -n systemctl enable --now peakrunner-ddos

**Inspect or remove:**

    sudo -n nft list table inet peakrunner_ddos
    sudo -n systemctl disable --now peakrunner-ddos     # deletes the table

**Verify** after install: rerun `public_smoke` against both ports and check
that the counters stay near zero for real traffic.
