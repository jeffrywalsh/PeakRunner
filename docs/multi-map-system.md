# Multi-map foundation — in progress

The six-map rotation is published. See
[release record](release-20260921-1.md) and the six-map section in AGENTS.md.
Broadside, Stonehenge, Snowblind, and Desert of Death are reference layouts
to rebuild. Current source loads them when their packs are installed.
Collection-wide admission and signed delivery are verified for those six maps;
the general installed-pack registry and selected-map-only admission remain open.

Development checkpoints have been consolidated onto main at the user's request.
Do not deploy until the complete pipeline is tested. Continue work in the original
checkout, creating a new focused branch from main when needed.

## Server authority contract

The server owns the configured ordered map/mode rotation and chooses the active
entry. Clients cannot vote, override, or submit a mode through movement/input
messages. A map's supported modes describe capability, not server policy. Only
implemented modes may be configured; CTF is the only supported mode for now.

Clients install map assets locally for fast rendering and matching collision
prediction. The server loads the same authoritative geometry/spawn data and
checks selected identity, revision/content fingerprint and mode at admission.
Future menu map selection is for offline play only; multiplayer follows the host.
Rotation changes must coordinate round state, asset readiness and late joins;
never silently substitute a missing map or inherit a previous map's world state.

Much later, enthusiast servers may deliver custom maps/mods. Reserve that as a
separate security/design milestone: bounded downloads, consent/trust policy,
content-addressed cache, validation and mod isolation. A hash verifies equality,
not trust. No automatic server downloads or arbitrary code execution now.

## First checkpoint

`core::map_catalog` defines a strict, portable metadata contract: stable lowercase
map IDs, display names, schema/revision, supported rules, team spawn regions,
extent and resolution. It rejects duplicate identities, unknown fields/modes,
nonfinite/out-of-bounds coordinates and excessive dimensions/catalog counts.
Only CTF is allowed until other game modes are implemented. Metadata contains
no paths, URLs or scripts. The catalog is not yet a runtime pack loader.

`MapId::key/parse` centralizes existing Valley/Raindance identities and server
selection. Existing display labels, enum serialization, gameplay protocol,
Raindance asset bytes and movement remain unchanged.

## Remaining integration, in order

### Server rotation checkpoint

Dedicated QUIC server accepts optional `PEAKRUNNER_MATCH_ROTATION` JSON:

```json
[{"map":"raindance","mode":"ctf"},{"map":"valley","mode":"ctf"}]
```

This overrides the initial `PEAKRUNNER_MATCH_MAP`. Omit it for the existing
single-map behavior. Rotation is bounded to 32 entries/8 KiB; unknown maps,
modes and fields fail configuration. Repeated entries are permitted as weighting.
The public server configuration has NOT been changed.

After intermission the server advances the rotation, preserves connection slots,
identities/teams, acknowledgements, rate limits and monotonic tick/round state,
and creates fresh world state. Stale queued inputs are discarded at transition.
Empty servers return to the first rotation entry. Status reports the active map.
Tests cover policy parsing, wrap/reset, world-transition invariants and two real
local clients receiving the server-selected starting map.

This currently uses both built-in maps and the old combined compatibility hash.
It does NOT yet provide per-selected-map admission, custom pack coexistence, or
mode/rotation discovery fields. Full live round-transition render/transport QA
is still required before merge. No public deployment or new release yet.

1. Replace the global optional `map_pack::PACK` with an immutable installed-pack
   registry. Keep built-in Raindance available when another pack is installed.
   Decide a stable wire identity (never process-local catalog indices), and bound
   total loaded assets as well as each file. Keep private legacy reference imports
   separate from distributable original packs.
2. Connect metadata to manifest/compiler output. Preserve existing terrain diagonal,
   hole, dimensions, spawn clearance and equipment behavior. Remove hardcoded
   Raindance assumptions in terrain/render/audio/cache selection. Metadata alone
   must not advertise a playable pack.
3. Make server-selected map identity plus content fingerprint part of admission.
   A different *unselected* installed map must not prevent joining. Reject missing
   or mismatched selected content before granting a player slot. Bump protocol
   only when implementing this coordinated wire change.
4. Present installed maps in the offline menu, and server-selected map/mode plus
   missing-content status in the multiplayer browser. Add server-owned validated
   rotation configuration (CTF only initially). Update signed launcher layout and
   packaging to distribute multiple packs, preserve old clients' update/rollback
   behavior, and keep directory independent of gameplay core.
5. Test two distinct original packs coexisting, switching and loading safely,
   matching and mismatching clients, invalid manifests, late joins, resets and
   rendered collisions/spawns. Run real multi-client and native render checks.

No new map, release artifact, server deployment or completed multi-map support
is claimed by the first checkpoint.
