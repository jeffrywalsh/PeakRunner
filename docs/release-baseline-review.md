# Release baseline reconciliation — 2026-09-19

This baseline records the already deployed `0.1.0-raindance.20260919.4`
release and the preceding project work, starting from `terrain-materials`
commit `0f1ab82dfe6a4f4d5d8ec9f29359c5a8ef45e12e`.

All 72 runtime/build/example/map files present in the shipped source archive
were byte-compared against this baseline. Archive SHA-256:
`f8ae31fe40ef6be449eb7860dc0f239667fa39d2fde37f2316d7f0dac227ee03`.
Deployment records retain their historical working-tree provenance: the
artifacts were built before this reconciliation, not from its eventual commit.

Verification: 128 workspace library tests passed (three explicitly ignored
GPU/load tests), all-target workspace checks passed, five original-map tests
passed, dependency-boundary checks passed, and website JavaScript checks passed.
Prior release visual/platform checks and limitations remain documented in
`visual-release-20260919-4.md`; no new Windows runtime test is claimed here.

Reviewed admission/control-message handling, server-owned names and team chat,
signed launcher manifest/path/hash checks, deployment configuration, and asset
provenance. Staged private-key/token-pattern and file-size checks were clean;
this is not an exhaustive security audit.

Excluded private extracted Tribes data, credentials and signing keys, generated
downloads, VM data, unrelated captures, and the later Find a Rift preferences
feature. Original dirty workspace contents remain preserved. Git publication
does not rebuild artifacts or restart the running services.
