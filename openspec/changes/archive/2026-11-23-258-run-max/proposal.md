# Proposal

## Why

`brd-host wait` (#258) waits for a run in bounded calls, but a run still started either in a call that a tool ends
(and whose SIGTERM cancels the run) or in the background with its id read from a file. An agent needs one bounded call
that starts the run and, for a run of minutes, also ends it.

## What Changes

- `brd-host run … --max <duration>` follows no output: after the pack it waits for the end as `brd-host wait` does, up
  to `--max` from the start of the call, finishes the run as a wait does and says its summary. Past `--max` it exits
  124 (`still_running`) without a cancel, and `brd-host wait <alias> <run>` goes on.
- The skill, docs/contributing/studio-runs.md and AGENTS.md say: `brd-host run <host> --max 9m -- <command>`, then
  `brd-host wait` while it exits 124; never a `timeout` wrapper, a run in the background with a marker file, or a
  follow loop.

Capabilities: `cli/host`.
