# Proposal

## Why

In the IDE the Settings window of Branch Deck showed an empty system band above its row: the traffic lights and the
title of the window. The desktop app draws the row beside the traffic lights. The IDE can now merge the row of a page
into the title bar of a window on macOS (x-air-app 1.1.0, additive), so the window has one row less.

## What Changes

- The app file asks for the merge for the Settings window: `settings.window.titleBar: overlay`.
- The page passes the title bar of its host context (`hostContext["x-air-app"].titleBar`) to `bd_bootstrap`. brd
  answers `overlay` with `doubleClick: "host"` and the height of the IDE for it, `native` for every other place.
- The page has no drag script of Tauri in the IDE: a press on the empty parts of the row is `x-air-app/windowDrag
  {clickCount}`, and the IDE moves the window or does the double click of the system. The IDE says full screen by the
  inset 0, and a change of the title bar resizes the window for the page.
- One Rust type owns the shape of the title bar for the desktop app and for `brd mcp` (`domain_types::title_bar`).

Capabilities: `repository/ide-embedding`, `repository/title-bar`.
