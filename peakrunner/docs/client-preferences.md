# Saved Find a Rift settings

The native client saves its player name, directory address and direct server
address in `client.json`, separate from the installation and updater generations.

| Platform | Default path |
| --- | --- |
| macOS | `~/Library/Application Support/PeakRunner/client.json` |
| Windows | `%APPDATA%\PeakRunner\client.json` |
| Linux | `$XDG_CONFIG_HOME/PeakRunner/client.json`, or `~/.config/PeakRunner/client.json` |

Linux ignores a relative XDG_CONFIG_HOME. If APPDATA is unavailable on Windows,
the fallback is the user's `AppData/Roaming/PeakRunner` directory. A missing user
configuration directory disables persistence without preventing play.

Edits save after 600 ms without typing, on joining/refreshing/leaving the browser,
and on normal shutdown. The last valid name/address is retained when a field is
blank or invalid. Hover the automatic-save hint to see the exact file path.
Malformed/oversized files fall back to defaults with a visible warning and are
not rewritten just by loading them. Save failures remain nonfatal and visible.

Only schema, name, directory and direct address are serialized. Match passwords,
sessions and chat are never saved. Addresses with URL credentials, query strings,
fragments or control characters are not persisted. Native transport and server
validation remain authoritative. Files are replaced using a same-directory
temporary file and rename, with 0600 permissions on Unix.

For isolated testing, set an absolute `PEAKRUNNER_CONFIG_DIR`. Never point tests
at a real user's profile. Automated `PEAKRUNNER_JOIN` sessions intentionally use
memory-only preferences, avoiding accidental replacement with a QA server/name.

This is a new, unshipped feature after release `.20260919.4`. It needs its own
tested main merge and subsequent client release; the already published binaries
do not gain persistence from changes to this source tree.
