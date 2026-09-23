# PeakRunner native playtest

Original Raindance, Tower Complex, Cairnhold, Frostline and Dustreach map
assets are embedded in the standalone executable. No Tribes install or separate asset download is needed.
Version .20260921.1 requires a matching server and map collection; older
.20260919.4 clients and servers are incompatible. This release adds saved Find
match preferences (never passwords) and server-selected CTF map rotation.
All five rotation maps are original: Tower Complex, Cairnhold, Frostline and
Dustreach replace the four earlier reference layouts, and Skybreak Bastions is
removed. No separate private map packs ship. Map traversal and balance still need human playtesting. Approved
movement constants are unchanged.

- macOS: Apple Silicon only. Extract the ZIP and open PeakRunner.app. The bundle
  is ad-hoc signed for integrity, not Developer ID signed or Apple notarized.
  If macOS blocks the unidentified developer, use System Settings > Privacy &
  Security > Open Anyway for this app. Do not disable Gatekeeper system-wide.
  A "damaged" warning is not expected; report it rather than bypassing it.
- Windows: x86-64 build. Extract the ZIP, then open PeakRunner.exe. Not code signed.
  Native Windows runtime verification is still pending; cross-compilation alone
  does not prove graphics, audio or input compatibility.
- Linux: x86-64, built against Debian 12 (glibc 2.36 or newer). Requires a desktop
  session and Vulkan-capable drivers, plus ALSA, udev and X11/Wayland libraries
  (including libxkbcommon-x11).
  Software Vulkan rendering is suitable for compatibility testing, not a promise
  of playable frame rates. Run the extracted peakrunner executable.

Select Find match to set your name and join a host. During a match, Escape opens
the match menu: edit Your name and choose Apply name. Current shows the server's
accepted name. Names allow 1–24 ASCII letters, numbers and spaces. Outer spaces
are trimmed; blank, punctuation, Unicode and control-character input is rejected.
The server permits one change every 10 seconds. Renaming does not reset scores
or identity. Display names are not verified accounts and need not be unique.

Standalone packages do not auto-update. The separate PeakRunner Launcher can
download verified updates without downloading unchanged map files.

Chat: T opens public chat; Y opens team-only chat. Enter sends; Escape cancels.
While chat is open, movement, weapons and other gameplay input are disabled.
The server validates both channels and filters team messages before transmission.
