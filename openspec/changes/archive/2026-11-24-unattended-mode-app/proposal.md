# Proposal

## Why

The user switches Unattended Mode where they work, in the app, and must see at a glance that it is on: agents are on
their own while it is. In the morning the app tells how many requests the night denied and shows them.

## What Changes

- The mark of the mode in the title bar of the main window while it is on, which turns it off; Turn On / Turn Off
  Unattended Mode in the command palette and the switch of Settings → General: one item for all
  (`set_unattended_mode`). Off, the title bar shows nothing of the mode.
- A switch on without the hook warns and offers Install Agent CLI; a switch off tells the count and opens Denied
  Requests, the list of what was denied.
- Settings → Tools says when the hook is missing or runs another Branch Deck.

Capabilities: `repository/unattended-mode`, `repository/command-palette`, `repository/settings`.
