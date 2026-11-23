# Proposal

## Why

Every client of the one backend (#260) shows the view of the session of a repository: the Tauri shell for its
webviews, the bridge for the views of the IDE. Today each window subscribes the session by `subscribe_view` and hears
its patches on a channel of its own call; over a connection that serves several windows the view is a channel of the
host, one per repository, with the `ViewPatch` as its typed action
([Backend Protocol](../../../docs/design/backend-protocol.md), "The repository channel").

## What Changes

- One channel per repository path for the life of the backend, `x-branch-deck/repositoryChannel` gives its URI.
- A subscription is a window's: `subscribe` names the window, the session builds its catch-up from `have` as now.
- Its state is the `RepositoryView` with its `Version`; its one action `x-branch-deck/viewPatched`, applied by
  `repository_model::patch::apply_patch`, one frame per subscribed window; `serverSeq` orders it.
- `subscribe` with `have` answers `held` and the catch-up patch first; another epoch and a slow subscriber are a
  resync on the same channel; the end of the session ends the subscription.
- The channel takes no intent; the demand is set by window, of its subscription.

Capabilities: `repository/backend-protocol` (new, with `260-backend-wire` and `260-backend-commands`).
