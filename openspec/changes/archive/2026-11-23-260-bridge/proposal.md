# Proposal

## Why

Each project of the IDE ran a backend of its own (`brd mcp`, P1): its own sessions, its own polls, another time of the
last check than the desktop app beside it. With one backend of the user (#260,
[Backend Protocol](../../../docs/design/backend-protocol.md)), the process of the IDE becomes the bridge
`branch-deck ide`: `x-air-app` of Air on its stdio, the protocol of the backend on the socket of the desktop app, which
it starts without a window when it does not run.

## What Changes

- `branch-deck ide` (parsed before Tauri; `branch-deck cli mcp` and `brd mcp` stay its alias) joins the backend of the
  app on its socket, or starts the app without a window (`--background`) and joins it: macOS through `/usr/bin/open`,
  Linux by a double fork, Windows detached out of the job of the IDE.
- Where the app cannot run (no display, another build with a window open, Windows while the job of Air refuses the
  breakaway, `BRANCH_DECK_IDE_BACKEND=in-process`), the bridge serves the IDE with a backend of its own process, the
  behaviour of P1, and ends with 1 once a backend of its build appears, so the IDE starts it again and it joins.
- The exit codes: 0 when the stdin ended, 1 when the backend ended first.
- The mirrors are subscriptions of the repository channels under a window of the bridge; a patch into another epoch
  reads the view anew; the places of the IDE are the windows of its connection; a news is claimed before it is told,
  so the backend tells it once.
- The app file names the app binary with `ide` on every platform, Windows too.

Capabilities: `repository/ide-embedding`.
