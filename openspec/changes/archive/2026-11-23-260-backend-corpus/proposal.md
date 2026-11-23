# Proposal

## Why

The golden corpus of the backend ran first on real sessions with the host (#260): a sync and an operation send several
patches, the session catches a window up only from the seed of its warm start, and the `done` of a rewrite names
another version than its answer. The specs and [Backend Protocol](../../../docs/design/backend-protocol.md) said less;
and the build id had no commit in any build.

## What Changes

- The catch-up of a subscribe is from the seed of the warm start alone; any other `have` but the version the session
  holds gets the whole state.
- A command is several patches; no client counts on one.
- Each answer and event is fenced to its own version; the other events of a stream have no order against the actions.
- The build id is `<version>+<commit>`, the commit `BRANCH_DECK_COMMIT` (`scripts/install-app.sh`), else `GITHUB_SHA`
  of CI, else `dev` (`crates/backend-host/build.rs`).

Capabilities: `repository/backend-protocol`.
