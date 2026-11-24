# Proposal

## Why

The short lane of the heavy queue (change heavy-turn-short-lane) puts a short run ahead of the long ones and counts
how often each long run was passed. `brd-host stats` should show that lane at work: how many runs were short and how
often runs were passed.

## What Changes

- `queue.json` of a heavy run gains `short_ms` (the wall that made it short) and `passes`, read from the marks of the
  queue (`passes.json`) at the looks of its wait; its place is the one the rule of the queue gave it.
- The summary counts the short runs and the passes of its heavy queue, and its line says them.

Capabilities: `cli/host`.
