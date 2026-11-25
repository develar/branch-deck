# Proposal

## Why

An idle lease whose only work is local changes kept its slot for ever: any local change of a gone holder made the lease
`idle_with_work`, and no get or reclaim touched it. On the idea-1 pool on 2026-11-25 two slots of the same gone holder
kept the probe changes of a research agent (41 paths and 2 paths) and no commit, so gets cloned new slots beside them.
The verdict of such a lease also held after the changes were restored by hand: `IdleCheck::holds` looked only at the
`HEAD` and the reflog of the slot, which `git restore` does not move, so only an explicit `brd lease reclaim` saw the
clean slot.

## What Changes

- A get and `brd lease reclaim` preserve the local changes of an idle lease as patches in its notes and take the slot,
  when the changes are its only work and no changed path was touched for `branchdeck.leaseIdleChangesAfter` (`12h` by
  default, `0` never). The changes stay where a patch can't carry them: an unmerged entry, a gitlink or a nested
  repository, a path with a filter, an entry of the index with assume-unchanged or skip-worktree, a lease without
  notes, patches past `branchdeck.leasePreserveMax` (`64M` by default).
- The patches: `changes.patch` (HEAD to the tree of `add --all` on a copy of the index) and `staged.patch` (HEAD to
  the tree of the index where it is neither), proven by an apply on a new index. The slot is only read until the commit
  point.
- The order: the record of the notes (`preserved`, `preserved_at`) before the commit point, the commit point (the
  record of the slot names the patches, the slot dirty) before the discard, the discard after the close of the notes.
  A second `status` that finds a path or its stat moved stops the takeover.
- The verdict of an idle lease with local changes holds by a stat proof of its paths, and until the changes are quiet.
- The answers name the patches: `took_over.preserved`, `reclaimed[].preserved`, `reclaimed.preserved` of the status,
  `preserved` of `brd lease notes`, the message of `lease_reclaimed`; `idle` names `quiet_at` and `not_preserved`.
- The notes of a preserve expire `branchdeck.leaseNotesKeep` after the later of the return and the preserve.

Capabilities: `cli/lease`.
