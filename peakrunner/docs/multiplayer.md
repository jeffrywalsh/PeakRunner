# Authoritative public and private matches

## Public deployment

The native client defaults to `https://dir.peakrunner.net/servers` and
`quic://play.peakrunner.net:7777`. These use validated public CA certificates;
there is no insecure certificate bypass or plaintext fallback. Gameplay uses
QUIC datagrams over direct UDP with TLS 1.3; reliable streams carry admission
and status only. HTTPS directory responses are bounded, redirects are disabled,
and public listings must use QUIC. Deployment is pending certificate provisioning
and WAN testing; the new public match is not yet live.

The intended split is a `directory` service on dellcon behind Cloudflare Tunnel,
and `peakrunner-quic` on the VPS with only UDP 7777 published. Cloudflare handles
HTTPS discovery, not gameplay. Gameplay encryption runs from native client to
VPS; the local authoritative backend uses trusted loopback TCP. The server
operator can read game state. Restore files and pending deployment gates are in
`deploy/dellcon/` and `deploy/vps/`.

Only operator-configured official servers are listed. The directory polls match
health and expires stale results; public registration/mutation endpoints do not
exist. The QUIC gateway uses address validation, caps connections (16 global,
8 per IP), admission bursts (12 per IP, replenishing one per three seconds),
input packet size (256 bytes), input rate, write duration and idle duration.
Inputs redundantly include three numbered frames. Snapshots are independently
replaceable and LZ4-compressed, bounded to 12 fragments and 64 KiB after
decompression, with at most four incomplete assemblies. Transmit queues are
16 KiB for snapshots and 1 KiB for inputs to avoid seconds of stale buffered state.
Old, duplicate and incomplete snapshots never block a newer complete snapshot.
Zero-RTT is disabled. The old WSS implementation is not the public gameplay path.

The directory and gateway use health checks; Docker restarts exited processes.
An unhealthy-but-running container needs operator attention (Compose alone does
not automatically restart it). Logs record joins, departures and phase changes
without passwords. Reconnect is available in the disconnect screen; it joins as
a new session, not a restored identity. Deployment/restore files are in
`deploy/dellcon/`; credentials stay outside Git.

## What is implemented

- Eight actual players, stable connection-assigned IDs (display names may repeat).
- Auto-balanced teams; one server-selected map and shared 60 Hz physics.
- Disc, chaingun, and grenade simulation, damage, health, energy, cooldowns,
  blast impulses, deaths, respawns, flags, captures, scores, and round clock.
- Warmup until both teams exist, three-second countdown, first to three captures
  or eight minutes, ten-second intermission, automatic restart. Late joining is
  supported. Disconnects drop carried flags and remove owned projectiles. Losing
  an entire team aborts the round into warmup with a fresh countdown on rejoin.
- Client-side movement prediction and input replay against acknowledged snapshots.
  Other players are interpolated. Projectile rendering is extrapolated between
  snapshots; clients do not resolve authoritative hits or captures.
- 20 Hz full snapshots, a Tab scoreboard, direct joining, optional directory,
  optional match password, and visible disconnect/rejection states.
- Focus loss and menus send neutral controls. The match never pauses for a client.

## Trust boundary

The server accepts a numbered control frame (movement axes, view angles, held
actions, weapon slot). There are no position, velocity, health, damage, score,
team-choice, or client-delta-time messages. Server time advances independently
of packet count. Inputs must be finite and in range with increasing sequence
numbers and bounded jumps; UDP loss does not require a disconnect. Rate, frame,
connection, and queue limits reject abusive peers.

Sockets are nonblocking with incremental framing; partial lines survive polls.
There are at most 16 connected/pending peers, 8 player slots, 12 queued inputs
per peer (newest sampled each tick), and 4 outgoing frames per socket. A 32-message
burst allowance absorbs jitter, while a sustained 120/s limit rejects floods.
Handshakes expire after 3 seconds;
missing inputs become neutral after 250 ms and disconnect after 5 seconds.
Snapshots cannot exceed 256 KiB. Slow consumers disconnect instead of building
unbounded queues. Directory listings are bounded and require an owner token
for updates/removal; the advertised IP must match the registering peer.

**Legacy LAN hosting is not encrypted.** Its passwords and directory lease
tokens travel over plaintext TCP. That transport is suitable only within
a trusted network, preferably a private encrypted VPN with restricted members.
Only specific loopback, RFC1918, VPN shared-address-space, or IPv6 ULA binds are
accepted. That is an accidental-exposure guard, not protection against port
forwarding, proxies, malicious VPN members, packet interception, or DDoS.
Do not use a password you use anywhere else. The server operator is trusted.

## Hosting

In the native game: select the map on the main menu, open **Find match**, expand
**Host a private match**, enter the host's specific LAN/VPN IP and port, and
choose **Host and join**. Friends use **Join directly** with that same address.
Loopback (`127.0.0.1`) is only for testing on one computer. A hosted match uses
the same authoritative server as the headless executable; closing the host's
match stops it for everyone. Hosting from the UI does not advertise a directory
listing automatically. Optional passwords use the Find match password field.

For a separate headless host:

Build from the `peakrunner` directory:

```sh
cargo build --release -p peakrunner-net
target/release/peakrunner-server --bind 127.0.0.1 --port 7781 --name "North Spine" --map Valley
```

Replace the loopback bind with the host's specific private LAN/VPN address to
allow friends to connect. The server is headless: its dependency tree contains
no GPU, windowing, or audio libraries. Linux/Windows binaries should be built
on those targets; this pass does not certify an untested OS build.

Set `PEAKRUNNER_MATCH_PASSWORD` in the server process environment for an optional
invite password. Players enter that password in Find match. Keep it out of
source control and do not expose the service directly to the internet.

Optional directory, on the same private network:

```sh
target/release/peakrunner-directory 127.0.0.1:7780
target/release/peakrunner-server --bind 127.0.0.1 --name "North Spine" --map Valley --directory 127.0.0.1:7780
```

Change both addresses for a LAN/VPN setup. `--advertise` may specify the server
IP explicitly; it must match the connection's source IP seen by the directory.
Listings heartbeat every two seconds and expire after eight seconds. There is
no automatic deployment or assumption that a previously used host is running.

Native clients optionally accept `PEAKRUNNER_JOIN=wss://play.peakrunner.net/match`
(or a private `IP:PORT`), `PEAKRUNNER_NAME`,
and `PEAKRUNNER_MATCH_PASSWORD` at launch for direct session entry.

The Dockerfile build context is the `peakrunner` directory, not `crates/net`.
Its default directory bind is loopback; configure a specific private interface
when operating a container. Do not publish it as an unprotected public service.

## Verification

```sh
cargo test --workspace
cargo test -p peakrunner-net eight_clients_sustain -- --ignored --nocapture
cargo test --release -p peakrunner --lib -- --include-ignored
cargo check --target wasm32-unknown-unknown
```

Tests cover shared snapshots, duplicate display names, all eight slots,
password rejection, malformed/oversized traffic, listing ownership, partial
frames, authoritative airborne movement, all three weapons' damage and respawn,
flag drops/captures, late joining, round restart, and delayed input reconciliation.
Existing movement/collision regressions remain in the shared core.

Local verification for this implementation: 62 tests passed including GPU
captures and the 12-second eight-client load test (about 60 server ticks/s;
largest observed snapshot about 24 KiB). Two native game windows were checked
for shared matches, live movement, the scoreboard, UI hosting, guest joining,
and host-disconnect handling. The native release build and WASM compile passed.
This is a short local check, not a long-running or cross-machine certification.

## Remaining before public competitive hosting

1. Verified player accounts and persistent reconnect identities if requested.
   TLS verifies the service, not the player. Public sessions are anonymous.
2. Compact binary/delta snapshots and latency/loss testing on real networks.
   The current JSON/TCP/WSS path prioritizes correctness and accessibility; TCP
   head-of-line blocking and snapshot bandwidth make it unsuitable to certify
   for competitive WAN play. There is no server rewind/lag compensation yet.
3. Operator tools: kick/ban, reconnect identity, team management, readiness,
   map rotation, structured match logs, and durable results if requested.
4. Longer multi-machine soak tests and profiling on low-end clients/servers.
   Local eight-client tests are not evidence of internet or cross-OS performance.

Server authority prevents clients from directly inventing movement or damage.
It does not prevent aim assistance, collusion, information cheats, or a dishonest
server operator. No anti-cheat or exploit-proof claim is made.
