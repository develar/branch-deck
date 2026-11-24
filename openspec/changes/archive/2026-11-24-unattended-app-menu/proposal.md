# Proposal

## Why

The user switches Unattended Mode in the command palette, Settings → General and the mark of the title bar; the menu
bar of the app, where an app keeps such a switch, has none, so its state is not visible from there.

## What Changes

- A check item "Unattended Mode" in the app menu of macOS after Settings…, in File after Settings… on Windows and Linux
  with the system title bar, and in File of ☰ under a custom title bar; checked while the mode is on.
- Choosing it sends `unattended_mode` to the main window, which switches the mode through the one item of the mode
  (`unattendedModeItem`, `set_unattended`: the hook first on, removed after off), with the toasts of the mode and the
  error of a switch on that fails.
- The main window checks the item for every value of the setting (`set_unattended_menu_checked`), whichever surface or
  process changed it; the item checks itself back after a click until then, so a switch on that fails leaves no check.

Capabilities: `repository/unattended-mode`, `repository/title-bar`.
