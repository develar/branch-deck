## Why

The page compares the versions that commands answer (`SyncAnswer.version`, `Outcome.versionAfter`) with the version of
its view, so the version of the session cannot stay inside `brd mcp`: the mirror carries it. The page emits its events
for the other windows through `bd_invoke` of `plugin:event|emit`, which `brd mcp` must tell the views.

## What Changes

- The state of a mirror holds the part `version`, and each change sets it last.
- `plugin:event|emit {event, payload, target}` through `bd_invoke` is `x-air-app/event`, to the view of the target
  window when it names one.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `repository/ide-embedding`: the two requirements of the mirror, and one requirement added to Events.

## Impact

- `crates/mcp-host/src/mirror.rs`, `crates/mcp-host/src/server.rs`, the comment of the corpus file of the app link.
