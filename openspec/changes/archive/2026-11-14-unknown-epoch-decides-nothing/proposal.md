# Proposal

## Why

The review of the epoch of a worktree found a loss of data: a read of the state of the pool that failed (an error of
the disk, a read during a rename on Windows, a state a later build wrote with a state of a lease this build does not
know) answered "no slot", and the writer of the settings forgot the entry of the lease the slot held and saved that,
stamped the next write with no epoch, saved frames under none and closed a live session for a window that came back.

## What Changes

- The epoch has three states (`WorktreeEpoch`): `NotASlot`, `Lease` (the id of the lease, `returned <n>` of a free
  slot) and `Unknown`. `Unknown` decides nothing: the writer forgets no entry and keeps the epoch an entry names, a
  frame seeds nothing and the session saves its frames under the epoch the frame names, a kept session stays.
- The epoch reads only what it needs of the state of the pool (the path of each slot, the id of its lease, its last
  return), so a state of a later build still tells it.
- The epochs are read before the mutex of the writer, and kept by the identity of the files: a read of an unchanged
  slot of an unchanged pool is two `stat` calls.
- Capability `repository/onboarding` and `cli/lease`: the scenarios of an epoch the files cannot tell.
