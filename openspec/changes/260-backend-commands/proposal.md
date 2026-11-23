# Proposal

## Why

The commands of app-api, its events and its desktop are of one process today: a window is a label, an event goes to
every window of the app, and the desktop is the one of the Tauri shell. With one backend for the app, the IDE and later
brd (#260), a call, a window, an event and a desktop are of a connection
([Backend Protocol](../../../docs/design/backend-protocol.md), "Root commands", "The windows of a connection",
"The Desktop of a connection").

## What Changes

- Every command of `for_each_command!` is the root command `x-branch-deck/<name>`, `execute` and `cancel_operation`
  too; it answers `{value}`, `{bytes}` or `{error}`; its stream is `x-branch-deck/callEvent` bound to its request id,
  before the answer; an answer that names a version V goes after the actions up to V.
- The channel commands (`subscribe_view`, `unsubscribe_view`, `set_demand`) are sent as `subscribe`, `unsubscribe` and
  `setDemand` of the window; `get_snapshot` stays a root command.
- A window is a connection and a label; the end of a connection forgets only its windows; a read that is one per app
  is one per window.
- The Desktop of a connection: the native one in the process, host requests and capabilities over the socket.

Capabilities: `repository/backend-protocol` (new, with `260-backend-wire` and `260-backend-repository-channel`).
