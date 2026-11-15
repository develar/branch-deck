# Proposal

## Why

The review of 5d6a0476a found the teardown of `cross_process_test` at 25 to 66 s where it took 1: a sync of the app
that a look started for the stamp of `brd` ran git while the test deleted its repository, and that git took 18 to 28 s
to fail with "Unable to read current working directory". git fails at once in a directory that is gone; one whose
directory goes away under it scans the parent on macOS (`getcwd`), here a temporary directory of 122 000 entries. A
window of a repository that went away has nothing to sync, and nothing closed its sessions.

## What Changes

- A look of the watch that finds the git directory of the repository gone closes the sessions that watch it
  (`Sessions::close_worktree`) instead of syncing; a stamp that went away asks no sync.
- The harness of `cross_process_test` looks every 50 ms (`Registry::watched_as`), and `Space` drops its runtime before
  its repository, so the blocking work of the app ends while the repository is there.
- The settings writer takes its first look from `dir_watch::watch_from`, before its watcher.
