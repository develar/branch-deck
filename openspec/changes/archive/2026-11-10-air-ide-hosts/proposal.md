# Proposal

## Why

The IDE now hosts its launch directories in its process, and it changes them while it runs. An instance file that the
IDE rewrites at each change of its projects and at each focus is a second copy of that state, and it can be stale. The
IDE now writes the file once at its start, and the client asks the IDE for its hosts over its socket.

## What Changes

- **BREAKING**: the instance file has only `schemaVersion`, `pid`, `product`, `build` and `socket`. The client does not
  read `projects` and `focusedAt` from it. The feature is not released, so there is no fallback.
- The client asks each running IDE for its hosts with the request `hosts` of protocol v1, all IDEs at once, each within
  2 s. The answer names the launch directories and the last focus of a window of the IDE.
- An IDE that does not answer in time, fails or refuses is skipped with a debug line.
- The pick rule stays: the deepest host first, then the newest `focusedAt`, then the newest instance file.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/start-in-air`: the instance file names the socket only, the IDE of a directory comes from the `hosts` answer
  of each running IDE.

## Impact

- `crates/air-launch/src/discovery.rs`, `client.rs`, `protocol.rs` and their tests; `fake_ide_test.rs`, the fake socket of
  an IDE that the client and the discovery tests share.
- `crates/app-api/src/app/air_launch.rs`: the doc of `AirIde.project`, and `src/lib/bindings.ts`.
- The Air plugin of the IDE writes the new instance file and answers `hosts` in the same release.
