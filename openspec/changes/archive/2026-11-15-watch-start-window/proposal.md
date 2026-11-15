# Proposal

## Why

bd-host-heavy found two gaps left by the looks of the watch of the operations registry (2026-11-15-watch-is-a-hint),
in the hang of `app_events_test`: `FSEvents` registers a stream after `FSEventStreamStart` returned and from "now", so
it drops a change of the first moments without telling it (a write 25 ms after the start, 3 of 40 rounds on notify
8.2.1), and the first look of the task, taken once the task ran, could take that change as its start, so it was never
told; and a focus read the registry alone, so a journal whose event was lost told Activity nothing until its next change.

## What Changes

- The first look of the watch of a repository is taken before its watcher starts (as in f743fcffc of bd-host-heavy).
- A focus of a window, an admission and the E2E fixtures look at the registry, the stamp and the journal (`Registry::look`).
- The harness of `cross_process_test` waits for the looks of the app, not for reads of its own; the history the
  disk tests give is the one `changed_since` gives for the roots of each slot.
