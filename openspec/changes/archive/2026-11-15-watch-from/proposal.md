# Proposal

## Why

The review of the watch of the operations registry (fd61e4920) and bd-host-heavy found the window between the start of
the watcher and the first look of its task: a change `FSEvents` drops in the first moments of a stream was never told
when the first look already held it. 5d6a0476a moved the first look before the watcher; nothing kept the next watcher
of the helper from opening the window again, and the journal told at a look had no fast test (only `app_events_test`,
which waited for it with no deadline).

## What Changes

- `dir_watch::watch_from`: the first look, then the watcher, in one call whose test holds the order; the registry watch
  starts through it.
- The test of the window (bd-host-heavy's, with the stamp beside the journal) replaces the one of 5d6a0476a: the
  periodic look tells a journal and a stamp written before the first poll of the task, on a runtime of one thread.
- `app_events_test` waits for `activity-changed` with a deadline.
