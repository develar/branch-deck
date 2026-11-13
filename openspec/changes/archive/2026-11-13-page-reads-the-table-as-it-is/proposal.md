# Proposal

## Why

A review of `stale-view-pending` found the same class on the page: it read the Patronus view itself, so of the table
the view was derived for, not of the table as it is, and decided a landing in TypeScript: the rest of a whole stack
(`StackLanding.rest`, the popover, the menus, the review scope), the count of the Safe Push of the row menu
(`getLandingBranchCount` over `isLandedWhole`) and "Landed with …" of the row and the preview (the view's `landedBy`).
In Rust a branch built on one the view doesn't know was not pending, a failed derivation of this table showed as a wait
for Patronus with no error, the step named no robot of an earlier state while pending, and the doc named the wrong
window. A test of the remote statuses compared two lists whose order between branches is no contract.

## What Changes

- The step of a whole stack carries what is left of it of the table as it is (`NextStepView.rest`); the page reads it,
  the count of a Safe Push of the step (`NextStep.lands`) and what landed a branch of `landedStates`. `isLandedWhole`
  and `getLandingBranchCount` are gone; `pnpm lint:arch` refuses such an export and a read of what the view says
  landed (`view-landing-read`).
- A branch is pending while the view doesn't know it or a branch it is built on; of a view whose derivation for this
  table failed, such a branch says that failure (`PatronusStanding::of_unknown`).
- `earlier_robot` is the verdict of the facts there are, also while Patronus is pending.
- The test of the remote statuses compares them by branch; `take_statuses` says their order holds only per branch.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/next-step`: the descendants of a branch the view doesn't know, the failure of the view of this table, what
  the page reads.
- `branch/patronus`: the dot of a merged robot of an earlier state while Patronus is pending.

## Impact

- `crates/domain-types/src/next_step.rs`, `crates/domain-policy/src/next_step.rs`, `crates/repository-model/src/derive.rs`,
  `crates/bd-remote`, `crates/sync-core/src/remote_status.rs`; `src/lib/bindings.ts`.
- `src/hooks/useStackNextStep.ts`, `src/hooks/useBranchContextActions.ts`, `src/stores/patronus.ts`, `src/lib/patronus.ts`,
  `scripts/lint-arch.mjs`; `docs/design/landing.md`.
