# Proposal

## Why

The desktop app and the IDE run two backends of one user (#260): `brd mcp` of each project and the process of the app
build their own sessions, scheduler and polls, so two windows on one repository disagree and each remote gets a poll
from each process. The process of the app becomes the one backend, an AHP-shaped host that its Tauri shell, the bridge
of the IDE and later brd connect to ([Backend Protocol](../../../docs/design/backend-protocol.md)). This change is the
connection: the wire, its errors, the local socket and the events of the backend.

## What Changes

- One JSON-RPC 2.0 object per line, AHP names where the meaning is AHP's, `x-branch-deck/<name>` for the rest.
- `initialize` negotiates the AHP version, names the build of each side and the capabilities of the client.
- The errors of the protocol are JSON-RPC or AHP codes with `data.reason`.
- The socket of a build's key in an owner-only directory (a named pipe on Windows), the peer check, `backend.lock`,
  `x-branch-deck/showMain` as the single instance, and the rule of another build: the stale side yields, stale being an
  executable whose identity on disk (device, inode, size, modification time) is no longer the one at its start.
- The events of app-api go to every connection, a page's stay inside its connection.
- A news is told once by the backend (`x-branch-deck/claimNews`); the desktop page claims nothing until it goes through
  the host, so until then the app and the IDE may both tell a news.

Capabilities: `repository/backend-protocol` (new). The repository channel is the change `260-backend-repository-channel`,
the commands, windows and the Desktop of a connection the change `260-backend-commands`.
