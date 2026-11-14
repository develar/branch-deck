# Proposal

## Why

A branch menu opened right after the sync that found a fixup never showed "Squash 1 Fixup…" when the plan of the fixups
answered after it opened: an open menu shows each item in its place as it is now and an item that came only at its next
open (`frozenMenuGroups`), so the list never moves under a resting pointer. Under load the plan answers later than the
menu opens; `review-plan.spec.ts` waited 30 s for the item twice, and a user who opens the menu at once sees no squash.

## What Changes

- While the plan has not read a fixup commit of the branch, "Squash Fixups…" holds the place of the squash in the branch
  menu, disabled, "Reading the fixups of the branch…"; the open menu shows "Squash 1 Fixup…" there once the plan
  answers. No placeholder after a read that failed.

## Capabilities

### Modified Capabilities

- `commit/fixups`: the item of the branch menu while the plan reads.
- `branch/branch-row`: the presence the page decides by its plan.

## Impact

- `src/hooks/useBranchContextActions.ts`, `src/lib/autosquash.ts`, `src/stores/branch-sync.ts`, `src/model/store.ts`,
  `tests/e2e/squash-fixups/squash-fixups.spec.ts`.
