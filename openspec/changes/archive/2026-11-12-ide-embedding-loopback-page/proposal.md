# Proposal

## Why

The contract of Air (`plugins/air/native/air-apps/protocol/` of idea-1) changed on 2026-11-12. JCEF of Chromium 150
lets a page open a loopback socket only when the page came over loopback HTTP itself (Local Network Access), so
`air-ide` now serves the host document and its WebSocket on one port of `127.0.0.1`, with the ticket of a view in the
fragment of the address. With it the open requests of Branch Deck were resolved: the badge, the progress and the
notifications go through brd only, the actions of the app file are tools that only the app sees, and the close of a
window of the page closes its place.

## What Changes

- The copy of the contract in `docs/design/ide-embedding/protocol/` follows idea-1: the page entry, `pageUrl` of a
  ticket, `x-air-ide/appResourceRead`, the flag `connected` of an instance, `toolWindow.titleActions` and the icon of
  an action, the one reason `sessionClosed` of `viewEnded`.
- The app file of Branch Deck gets the actions Command Palette and Settings beside Sync, and the title of the tool
  window shows the three.
- `tools/list` of `brd mcp` lists the tools of the actions, `sync` and `open_command_palette`, only for the app.
- `plugin:window|close` of a page is `x-air-app/closeView` of its instance.
- The page sets the badge, the progress and its notifications only through `bd_invoke` of their plugin calls.
- Remote development of a split IDE is a non-goal.
