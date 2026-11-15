# Proposal

## Why

A session opened while the epoch of its slot could not be told (`WorktreeEpoch::Unknown`) never opened anew once the
epoch became readable: a known lease counted as another lease only against another known one. While it lived every
write of the lease was refused (the mark of a sync, the landed state, `lastSyncTime`, `lastBranchCount`, the record and
the dismiss of an update), so the slot never showed "Last synced" and was Ready to Sync again after a restart (the
probe of the review of a58b98da6: opened once, never marked, through Manual, Stamp, Focus and Manual).

## What Changes

- A session that opened under no lease it knows is of another lease once its slot reads one
  (`session::holds_another_lease`, the one rule of the run, `lease_began`, and of a stamp, `Actor::leased_anew`): the
  first run or stamp that finds it opens the session anew under that lease, and its next sync writes under it.

## Capabilities

### Modified Capabilities

- `repository/onboarding`: a session of a lease the files could not tell opens anew once its slot reads a lease.

## Impact

- `crates/repository-model/src/session.rs`, `crates/repository-model/src/runtime.rs`,
  `crates/repository-model/src/session_test.rs`, `docs/design/git-operations.md`.
