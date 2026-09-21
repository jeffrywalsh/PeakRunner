# Local UTM compatibility lab

Launcher continuation (2026-09-19): Windows x64 signed installation succeeded and
both the launcher and external-map `.3` game rendered in Windows 11 ARM. The
launcher required the wgpu backend: the VM exposes only OpenGL 1.1. Screenshots:
`launcher-windows.png`, `launcher-windows-game.png`. The game's independent lock
test passed. UTM/session instability interrupted the corrected PowerShell wrapper
retest; session-0 rendering failed. These are limited compatibility checks, not
full Windows gameplay certification. Two UTM copies were found running; the idle
duplicate was closed. Do not run both copies when resuming this lab.
Windows was subsequently shut down cleanly through the guest; both PeakRunner
test VMs and the unrelated Windows XP VM were confirmed stopped at handoff.

The test lab lives on **AllOfIt**, not inside this repository:
`/Volumes/AllOfIt/PeakRunner-VMs/`. Do not commit VM images, installer media,
cloud-init seeds, generated account passwords, TPM state or SSH private keys.

## Machines

| VM | Guest | Allocation | Purpose |
| --- | --- | --- | --- |
| PeakRunner Linux x64 | Debian 13 amd64 + XFCE | 1 emulated core, 6 GiB RAM, 80 GiB sparse disk | Run the actual Linux x64 release |
| PeakRunner Windows 11 ARM | Windows 11 25H2 ARM64 Pro | 4 virtualized cores, 8 GiB RAM, 128 GiB sparse disk | Run the actual Windows x64 release through Windows emulation |

The existing Windows XP VM is unrelated and must not be changed.
The external lab directory is mode 0700 so other local accounts cannot read its
unattended setup credentials or VM state.
Both test VMs use NAT, not bridged networking, and have no host directory share.
Linux SSH is forwarded on **127.0.0.1:2222 only**. Its `tester` account accepts the
Mac owner's existing public SSH key and has guest-only passwordless sudo. The
desktop auto-logs into that disposable account. Windows has a separate random
local test password in `windows-test-account.txt` (mode 0600) beside its VM.
Never copy these seeds/credentials to public hosting or Git.

These are **compatibility checks, not gaming benchmarks**. Linux x64 is CPU
emulated on Apple Silicon. Windows ARM runs x64 code via Windows' translation.
UTM's standard virtual graphics do not substitute for real Windows/Linux GPUs.
Native x64 hardware testing remains necessary for a supported-platform claim.
Windows must be licensed by the owner; downloading an ISO does not grant a license.
Setup uses the normal "I don't have a product key" option. No hardware-check
bypasses, activation cracks, or UAC disabling are part of this lab configuration.

## Creation / provenance

Read the scripts before running them. They refuse existing VM bundles.
Homebrew `qemu-img` is required; UTM's bundled `qemu-img.framework` is a shared
library, not an executable. The setup was made using UTM 4.7.5.

- `scripts/create-linux-test-vm.py`: verified official Debian image, isolated
  disk, cloud-init desktop install, SSH public key only.
- `scripts/create-windows-test-vm.py`: verified official Microsoft ARM ISO,
  dedicated blank disk, unattended setup and UTM guest drivers. The installer
  formats **guest disk 0 only**; never attach real host disks to this VM.
- `deploy/setup-windows-qa.ps1`: download and verify the public Windows build,
  extract it and add a desktop shortcut. Run inside the Windows test guest only.
  The QA disc was `D:` after installation (`E:` was the Windows installer).
  Inspect the label `PEAKRUNNER_QA`; don't assume Windows PE drive letters persist.
- `deploy/PeakRunner-QA.desktop`: Linux desktop shortcut for the extracted build.
- `deploy/vm-test-dns.conf`: guest-only DNS override if UTM's forwarded host DNS
  fails; install under `/etc/systemd/network/10-netplan-enp0s1.network.d/`.
- `scripts/configure-linux-vm-display.py`: stopped-VM QXL display configuration,
  preserving the previous configuration. QXL is not a confirmed black-screen fix.

Official sources:

- <https://cloud.debian.org/images/cloud/trixie/latest/>
- <https://www.microsoft.com/en-us/software-download/windows11arm64>
- <https://getutm.app/downloads/utm-guest-tools-latest.iso>
- <https://docs.getutm.app/guides/windows/>

The creation scripts pin the SHA-512/SHA-256 verified on 2026-09-19. If upstream
`latest` changes, fetch the new official checksum and review it; do not bypass
verification. ISO/media files live in the external lab's `Media/` directory.

## Management and QA

Open the `.utm` bundles from AllOfIt in UTM. Shut guests down before unplugging
the disk. Don't run two UTM copies simultaneously; this Mac has both `UTM.app`
and `UTM 2.app`, which can confuse Apple-event routing. Configuration automation
also stalled during setup, so don't rely on it without checking results.

If the Windows ISO falls into the UEFI shell, use
`fs0:\efi\boot\bootaa64.efi` and immediately confirm the boot-from-disc prompt.
`fs0:` is the small EFI boot partition, not the full ISO filesystem; its
`efi\microsoft\boot` path is not available. Inspect the mapping rather than
assuming the host's mounted ISO tree is what UEFI can see.

Linux: `ssh -p 2222 tester@127.0.0.1`. Do not enable password SSH or bind the
forward to all interfaces. Root SSH remains disabled in this guest.

Both guests needed explicit DNS because UTM's host-forwarded resolver failed.
Windows uses 1.1.1.1 / 1.0.0.1 on its active virtual adapter, set with
`Set-DnsClientServerAddress`; the Mac's DNS/VPN was not changed.
After installing UTM guest tools, `utmctl file pull` works for guest QA logs.
`utmctl exec` returns immediately on this setup; verify output files/results
rather than treating its zero exit as command completion. Use absolute Windows
executable paths and avoid reading credential-bearing files into tool output.

Temporary Windows QA QMP controls bind **127.0.0.1:4445 only**, never the LAN:
`scripts/vm-qmp.py status`, `key ret`, `type 'text'`, or `screenshot <path>`.
`scripts/configure-windows-vm-control.py` requires both guests stopped and UTM
closed, preserves the original config, and adds this endpoint. UTM's sandbox
rejected Unix sockets on the external volume. This endpoint is unauthenticated
and gives local processes control of the disposable guest; no host folders are
shared. Remove the additional argument after installation/automated QA.

QEMU screenshots use PPM (this UTM build lacks libpng support). Convert a capture
with `sips -s format png <capture.ppm> --out screenshots/<name>.png` and inspect it.

## Verification recorded 2026-09-19

- Debian desktop installation completed; `dpkg --audit` was clean.
- Public Linux archive SHA-256 matched
  `807c2901d18814e4509925ce0fd31ec95cd5d77998633156a43ef05a8f5e2df2`.
- The downloaded Linux binary rendered the Raindance menu in the XFCE session.
  Adapter: Mesa 25.0.7 llvmpipe / LLVM 19.1.7, Vulkan CPU renderer. Captured in
  `screenshots/vm-linux-game.png`. This is not yet full interactive/multiplayer QA.
- Windows 11 Pro ARM64 installed with UTM guest tools; the downloaded Windows x64
  release passed its pinned SHA-256 check and rendered the Raindance menu.
  Virtual display: Red Hat VirtIO GPU DOD controller, driver 22.7.38.43. Captured
  in `screenshots/vm-windows-game.png`. This does not establish native x64 GPU
  performance or full interactive/multiplayer correctness.
- Temporary localhost QMP listener removed and both Windows installation CD
  drives detached after setup. Their image files remain on AllOfIt for recovery.
  Both guests were shut down cleanly before this configuration cleanup.
- Windows subsequently passed a normal reboot. Linux rebooted with working SSH,
  DNS, XFCE and a clean package audit, but its UTM viewer stayed black even while
  a guest framebuffer capture showed the game rendering. A later resolution
  change stalled guest/control access; both disposable VMs required forced stops
  (Windows was idle). No user VM or host disk was modified.
- Retesting Linux with QXL instead of virtio-vga still produced a black UTM
  viewer. Restarting LightDM did not resolve it. Linux's interactive VM display
  remains unresolved; do not mark this lab fully ready or infer a game defect
  from that viewer symptom. `screenshots/vm-linux-guest-display.png` records the
  working in-guest framebuffer before the display-adapter change.
- The QXL retest also lost SSH/control responsiveness after a LightDM restart;
  the final clean shutdown attempt failed, and Windows' subsequent start stalled
  in the shared UTM session. Only these two new test VM processes were stopped.
  A further Windows recovery boot was therefore not verified. Preserve the disks
  and investigate UTM/session reliability before continuing interactive QA.

Before marking a build tested, verify the published archive checksum, launch
the actual downloaded binary, inspect the rendered menu and a match, check
input/chat/name changes/Leave Rift, and test encrypted WAN joining. Record the
actual graphics adapter, errors and limitations; a booted VM alone is not a
passing game test. Do not run disruptive capacity tests against occupied matches.
