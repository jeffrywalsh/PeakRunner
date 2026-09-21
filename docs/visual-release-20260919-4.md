# Visual release .20260919.4

Client-only release: original modular character armor and system-light-theme
menu contrast correction. Movement, collision capsules, map content, gameplay
protocol and server rules are unchanged. Springdale Central may correctly report
`.20260919.3` while clients report `.20260919.4`; those versions interoperate.
The user subsequently requested a matching server update and approved restarting
with one player connected. Springdale Central now reports `.20260919.4`, with
the same gameplay protocol. Its previous `.3` image and configs are retained in
`/opt/peakrunner/backups/20260919-visual4/` for rollback.

Launcher `0.1.0-r1` remains unchanged. Signed game feed sequence is `2026091907`.
The upgrade reuses all seven unchanged map files and replaces the game executable.
Standalone archives are versioned separately; retain previous archives and feed
manifests. A feed rollback needs a newly signed, higher-sequence release.

Source snapshot (working tree, not a clean Git release):
`/data/peakrunner/releases/20260919-visual4/source.tar.gz`, SHA-256
`f8ae31fe40ef6be449eb7860dc0f239667fa39d2fde37f2316d7f0dac227ee03`.
Linux artifacts use `deploy/launcher-linux.Dockerfile`, artifacts target, with
the existing build-cache base image
`sha256:89ab291a5b3e681948845e7cc6e6ba4efedde8e95220e8161353e98db9d8d07e`.
The private signing key stays on the administrator machine, outside all archives.

Verification before publication:

- Workspace library tests and all-target checks passed; application dependency
  boundaries passed. WASM check passed with the existing unused Lobby warning.
- Native GPU character/jet captures inspected; light-mode menu render inspected.
- Mac signed install, actual game launch and independent update lock passed.
  Upgrade from `.3` reused all map files. Extracted standalone app signature passed.
- Windows x64 release compiled. The test VM had no interactive user session, so
  this release has no new Windows runtime claim. The VM was shut down afterward;
  all three UTM VMs, including the untouched Windows XP VM, were confirmed stopped.
- Real two-client, ten-second encrypted WAN test against the existing `.3`
  server passed: maximum ack gap 9 ticks, snapshot gap 103 ms, 196 snapshots per
  client. No real players were present. Server was not restarted.

Published verification: all three signed public feeds and every blob passed the
pinned-key verifier. All six public archive hashes matched the website manifest.
A fresh Mac installation from the public feed launched and held its update lock.
Linux signed installation, actual managed-game lock and forced-light menu capture
passed in an isolated Xvfb/Mesa container. Desktop/mobile website screenshots were
inspected; six download cards, correct .4 label, no browser errors or overflow.
The website, directory and public server are healthy. Only the website and the
user-approved match server were recreated; directory and tunnel were unchanged.

Exact published artifacts are recorded in `deploy/launcher-release.json`.
Compilation and VM checks do
not constitute native x64 graphics-performance, audio, or full-gameplay testing.
