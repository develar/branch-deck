# Proposal

## Why

`brd-host wait` and `run --max` put the output of a run into `<git dir>/branch-deck/host/runs/<run>.log` and tell the
agent to read it; `apply_failed` keeps the patch beside it. The git directory of a slot of the lease pool is
`.git/worktrees/<id>` of the main checkout, so every read of a log of a run of an agent by its lead asked the user, as
the notes of the lease did (change lease-notes-in-pool).

## What Changes

- In a lease whose `notes.local` links to the notes of the lease, the log and the patch of a run go to
  `notes.local/brd-host/`, in the pool directory, and the summary names that path; they expire with the notes.
- The record, the end and the lock of a run stay in the git directory; any other checkout keeps everything there.

Capabilities: `cli/host`.
