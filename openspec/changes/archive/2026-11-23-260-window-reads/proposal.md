# Proposal

## Why

With one backend for the desktop app and the IDE (#260) a window is a connection and a label
([Backend Protocol](../../../docs/design/backend-protocol.md), "The windows of a connection"): the main window of the
app and the main view of the IDE are two windows. The request for branch names and the search of a conflict fix were one
per app, so a request of one window superseded the one of another, the IDE's that of the app.

## What Changes

- A window of app-api is its connection and its label (`WindowId`); the desktop of a command is the one of the
  connection of its window, the events of the app go to every connection.
- The request for branch names is one per window: a newer request of the same window supersedes it, a request of
  another window never does; the close of the window supersedes it.
- The search of a conflict fix is one per window: a new search of the same window and its cancel stop it; the close
  of the window too.

Capabilities: `branch/branch-name-suggestions`, `branch/conflict-fix`.
