# Proposal

## Why

The IDE starts the default of its New Session button when a launch names no target, and it names a session by its
local session id, the key of its row. The dialog and the toast follow both.

## What Changes

- The agent picker offers "Default Agent" first; its launch names no target.
- The session key of the page is `localSessionId`: the answer of a launch, the watch and its stop.
- The watches of one IDE share one connection: the toast follows the row of its session until the row is removed
  (`gone`) or the IDE closes the connection. The end after 4 h and `NO_SESSION` are gone.
- The dialog words the refusals of local AHP: `UNAUTHORIZED`, `BAD_PARAMS`, `NO_DEFAULT`, and the refusals of the
  consent.

Capabilities: `branch/start-in-air`.
