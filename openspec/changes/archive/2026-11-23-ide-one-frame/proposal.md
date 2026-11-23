# Proposal

## Why

The tool window of the IDE showed Branches, Reviews, Landing and Activity as four tabs, each a JCEF browser with its
own page, stores and subscription. Its title repeated Sync, Command Palette and Settings, which the row of the page
has. The page showed the Activity button beside the tab of Activity. The desktop app shows one window with its own
switcher and side panel.

## What Changes

- The tool window has one frame, the main view, as the main window of the desktop app: its switcher of Branches,
  Reviews and Landing, its Activity button and side panel, its Sync and its Settings.
- The app file has no tab of Reviews, Landing or Activity, no `tabTitle` and no `titleActions`. Activity opens as a
  window of the IDE where the tool window has no room for the side panel.
- `bd_bootstrap` gives no `viewTabs`; the page has no switch to another tab, and `brd mcp` has no
  `plugin:air|show_view`. The focus of the main window and Show in Branch Deck still show the tool window.
- The text of the progress shows in the title of the tool window ("Branch Deck: Safe Push 38%").

Capabilities: `repository/main-views`, `repository/ide-embedding`.
