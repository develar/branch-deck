# Proposal

## Why

The review of d6822cdc found the window showing a head it had not read: when the late view of the session that turns
the review plan on brings a head a sync moved meanwhile as well, the switch let go of the hold (`lastReady.experimental`
stayed false, no ready render with the plan came between), and the diff view emptied while the new files and diffs
were read, against the requirement that a moved head keeps the review on screen until they are.

## What Changes

- The switch of the review plan lets go of the hold only for the review on screen; with a moved head (another view) it
  holds as any moved head does.

## Capabilities

### Modified Capabilities

- `branch/branch-review`: a moved head that comes with the switch of the review plan keeps the review on screen.

## Impact

- `src/hooks/useBranchReview.ts`, `tests/e2e/branch-review/review-tour.spec.ts`.
