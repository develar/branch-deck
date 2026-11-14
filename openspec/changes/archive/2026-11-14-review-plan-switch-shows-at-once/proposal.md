# Proposal

## Why

`review-tour.spec.ts` "should show the uses left outside while the index is still asked the tests of the steps…" failed
1 in 16 runs under load: "Finding the tests of the change…" never showed, the overview empty. The session of a review
window tells it the review plan (`branchdeck.reviewExperimental`) with its view; on a loaded machine that view came
after the window had read and shown its review without the plan. The window took the switch for a next review and kept
the one it showed, without the plan, until the plan was whole: while the index was asked the tests of the steps (cold,
seconds), the user saw no overview, no uses left outside and no steps.

## What Changes

- The review the window keeps while the next one is read is kept for a moved head, another part or Mark Reviewed, never
  for a switch of the review plan: the plan of the review on screen shows as it is read.

## Capabilities

### Modified Capabilities

- `branch/branch-review`: a switch of the review plan shows the plan of the review on screen at once.

## Impact

- `src/hooks/useBranchReview.ts`, `tests/e2e/branch-review/review-tour.spec.ts`.
