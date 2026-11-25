# Proposal

## Why

A get that adds a slot by a clone ran two full `status` of the slot under the clones lock: the `status` of the clean
of the clone (`remove_untracked`), then `slot_changes`. The first one could not write the index of the slot, so its
untracked cache and its fsmonitor token were lost, and the second one walked the tree again. On the idea-1 pool at
load 150-190 on 2026-11-25 this second walk took 1 min 45 s to 4 min 30 s of each add.

## What Changes

- Every `status` of `remove_untracked` may write the index of the slot (`GIT_OPTIONAL_LOCKS=1`): the clean of a clone,
  and the discard of a return and of a give-back of a host slot.
- `remove_untracked` answers the local changes of its last `status`, and the add of a clone uses that answer: one
  `status` per add, told as the step `looking for local changes` before the clean.

Capabilities: `cli/lease`.
