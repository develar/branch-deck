# Proposal

## Why

The supervisor of a run waits for a host slot in looks of 2 s, so a cancel ends the wait, and each look called
`lend_waiting`, which started a watch of `branch-deck/` of its own: a new stream of file events of the system every
2 s of a wait of up to an hour. The wait for the heavy turn keeps one watch for its whole wait (host-heavy-runs-review).

## What Changes

- `lend_watching` takes the watch of a whole wait (`watch_lends`), started before its first look; `lend_waiting` is
  `lend_watching` with a watch of its own. The supervisor starts one watch per wait.
- Capability `cli/lease`.
