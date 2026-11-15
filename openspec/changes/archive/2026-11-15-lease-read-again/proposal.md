# Proposal

## Why

A session that opened while the lease of its slot could not be told is of no lease it knew. Once the slot reads one,
every intent of its window (Update Project, an undo, a dismiss) was refused with `plan_changed` ("the slot holds another
lease than the view it was asked on") until a run or a stamp opened the session anew: for a worktree never synced, until
the user pressed Sync. The words misled too, as the lease may be the same.

## What Changes

- The refusal of an intent of such a session has a code of its own, `lease_read_again` (exit 3, `Retry::AfterReread`),
  its words "Branch Deck could not tell the lease of this worktree as it opened it and reads it again now: nothing of
  … was done; … again" (`AnotherLease::ReadAgain`, `witness::lease_read_again`).
- The refusal opens the session anew under the lease read at once (`Message::LeaseRead`), as a stamp that finds it does,
  so the window moves to the view of that lease and the same ask runs on it. A session of another known lease keeps
  `plan_changed` and opens anew at the stamp of that lease.

## Capabilities

### Modified Capabilities

- `repository/onboarding`: an intent of a session that knew no lease of its slot.

## Impact

- `crates/domain-types/src/error.rs`, `crates/repository-model/src/{session,runtime}.rs`,
  `crates/operations/src/witness.rs`, `src/lib/bindings.ts`, `docs/contributing/agent-cli.md`,
  `docs/design/git-operations.md`, their tests.
