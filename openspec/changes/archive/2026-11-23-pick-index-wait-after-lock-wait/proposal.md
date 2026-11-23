# Proposal

## Why

Since a pick whose witness missed after a wait for another holder of the repository lock checks under that lock at
once (2026-11-23), its moves came after no wait for the index lock: the holder was most likely another pick, whose move
of the target starts the `git status` of an IDE, which holds `index.lock` for 11 to 20 s on the IntelliJ monorepo. The
move then waited for git's lock under `branch-deck.lock` (up to 30 s per write), so every other writer waited behind
it, and a lock past 30 s answered `locked` where the pick would have waited outside. The rounds that ran out of checks
without the lock had the same gap. The requirement "An index lock that appears under the repository lock sends the
pick back" held only for the rounds without the lock.

## What Changes

- A check under the lock whose moves meet the index lock of another process (`PickPlan::index_held`) releases the
  repository lock, tells the step `waiting for the index lock of another process, outside the repository lock`, waits
  for git's lock outside it (`wait_for_index_locks`) and checks under the lock again; after `CHECKS_WITHOUT_THE_LOCK`
  such waits the moves wait as before. A lock of the caller (`held_lock`) is not released.

Capabilities: `cli/pick`.
