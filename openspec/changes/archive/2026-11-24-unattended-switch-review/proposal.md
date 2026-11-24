# Proposal

## Why

A second review of Unattended Mode found seven faults of its switch, its hook file and its menu item:

- A save that failed after the setting changed in memory answered an error and dropped the change: the writer held on,
  the hook was in the file, the windows and the menu showed off, and a later save put on into the file, where the hook
  denied everything while the page said off.
- `brd install --uninstall` with the mode on removed the hook and left the mode on: the mark said on, nothing denied,
  and the night stalled at the first prompt.
- Nothing kept two processes from switching at once: an on of the app and an off of `brd` could leave the mode on
  without its hook, and a writer decided by its copy, which another process had changed on the disk.
- An on whose journal of the run before could not be removed left the hook it had added.
- The removal gave the settings of Claude Code back byte for byte only in the layout of Claude Code; a `"hooks": {}` or
  `"PermissionRequest": []` the file had went with the hook; a hook of the user in the entry of ours was written anew.
- A Claude Code older than the exec form of a hook (`args`, 2.1.139) runs `brd` alone, which denies nothing.
- With no main window (closed while an IDE keeps the app) the menu item switched nothing, and a switch of `brd
  unattended` left its check stale until the main window came back.

## What Changes

- A save that fails puts the setting back and takes away the hook a switch on added.
- `brd install --uninstall` turns the mode off through the switch of `brd unattended off` (`unattended_off`).
- A switch, and the hook refresh of `brd install`, run under the lock `unattended.lock` of the app data directory, the
  writer reading the files first (`SettingsService::flush_and_reread`).
- An on whose journal cannot be removed takes the hook away.
- An edit keeps the indent and the final line break of the file; the containers the file had empty stay
  (`unattended-hook.json`); the hook of Branch Deck is always an entry of its own.
- An on reads `claude --version` and refuses a Claude Code older than 2.1.295 (`onFailure`) with `install_refused`.
- The item follows every change of the setting from Rust, and a click with no main window switches through the core.

Capabilities: `repository/unattended-mode`.
