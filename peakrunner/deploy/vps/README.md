# Direct UDP match host

The existing ColoCrossing host is `root@198.12.80.145` (`dorkenheimer.com`):
Ubuntu 24.04, one shared vCPU, 961 MiB RAM, 20 GiB disk. This is a candidate
for one eight-player match, not a measured capacity guarantee.

Verified preparation: OS package updates installed; Docker and Compose installed;
key-based SSH verified after applying `00-peakrunner-hardening.conf`; password
and keyboard-interactive SSH disabled; UFW permits TCP 22 and UDP 7777 only.
Reboot completed and kernel `6.8.0-139-generic` verified; SSH, Docker and firewall
survived the restart, and no further reboot is required. The eight-player
`North Spine` match is deployed as `peakrunner-match-match-1`.

## Deployment and verification

1. Completed: reboot and verify SSH, firewall, Docker and the updated kernel.
2. Let's Encrypt certificate issued for `play.peakrunner.net`; DNS-01 uses a
   separate token scoped to peakrunner.net DNS Write + Zone Read. The broad token
   remains on the administrator machine. Automatic renewal dry-run passed;
   `certbot.timer` is enabled. Initial expiry: 2026-12-18 (renewals change this).
3. Built on dellcon from the revision in `source.json`, transferred over SSH,
   and pinned by immutable image ID in `release.conf` on both hosts.
4. Certificates are mode 440, root:10001, inside a protected directory mounted
   read-only. No credentials or private keys are in Git.
5. The health check observes an advancing simulation clock. Only UDP 7777 is
   published; internal HTTP status and loopback TCP backend are not public.
6. Pre-DNS WAN testing used an IP override while retaining normal hostname/CA
   validation. A native desktop join was visually verified. The final normal WAN
   run passed with eight clients for 60 seconds: 1166–1181 snapshots per client,
   longest snapshot gap 188 ms, maximum acknowledgement gap 18 ticks (~300 ms).
   Eight clients also passed
   60 seconds with 1% packet loss and 10–60 ms added delay in each direction:
   longest snapshot gap 410 ms, maximum acknowledgement gap 41 ticks (~683 ms),
   850–887 complete snapshots per client. This is a short resilience check, not
   proof of competitive feel or maximum capacity. CPU samples reached ~87% of
   the single vCPU, so leave the match capped at eight for human capability tests.
7. `play.peakrunner.net` is DNS-only A → `198.12.80.145`; the HTTPS directory
   remains on dellcon. Apex/www remain on the tunnel's maintenance response until
   the separate entry-page work is done. Old DNS answers may linger for their TTL.

See [OPERATIONS.md](OPERATIONS.md) for rebuild, restore and renewal procedures.

Compose restarts exited processes, not unhealthy running processes. Certificate
renewal requires a coordinated server restart; do not interrupt an active match
without notice. Direct UDP exposes the VPS IP: provider DDoS coverage and bandwidth
allowance still need confirmation. Encryption is not protection from aimbots.
