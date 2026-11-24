# Proposal

## Why

The bridge of the IDE (`260-bridge`) is the first socket client of the backend. It follows the prefix of the git
config as a window of the desktop app does when it subscribes again, without ending the subscription of its mirror; it
shows its windows itself; its main view takes the warm frame of the root of its project, not of the project the app
selected; and the life of an app without a window counts it.

## What Changes

- `x-branch-deck/followPrefix {channel, window}`: the channel takes the session of the prefix of now, every window of
  it moved by a resync before the answer when that is another session.
- `x-branch-deck/bootstrap` takes the `repositoryPath` of the main view.
- A client without `showWindow` among its host requests gets none (the bridge declares `openWindow` alone).
- The host counts its clients that initialized, the app's own connection left out (`Host::clients`).

Capabilities: `repository/backend-protocol`.
