# Proposal

## Why

The contract of Air (idea-1, `plugins/air/native/air-apps/protocol/`) keeps each mirror as one table channel of the page
link, `x-air-ide:/appView/<n>`, which every view of the mirror subscribes to. Its end reaches each view with the reason
of the app, `sessionClosed`, or with `serverExited` when the process exits, before Air fails the calls in flight. A
client reads a reason it does not know as `closed`. The shim already treats every end alike, but no test pinned it.

## What Changes

- The requirement "Page: The shim maps the Tauri calls onto x-air-app" gets the scenario of the end of a mirror: for
  each reason, the page asks `brd mcp` for the snapshot, and a late change of the ended subscription reaches nothing.
- The copy of the contract follows idea-1; `brd mcp`, its corpus file `branch-deck-app.ndjson` and the page do not
  change.
