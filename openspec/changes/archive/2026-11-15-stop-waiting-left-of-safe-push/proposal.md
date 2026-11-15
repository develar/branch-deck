# Proposal

## Why

Stop Waiting came on the right of the Safe Push button of a row as its start began to wait for the repository lock. The
step track stands at the end of the row, so the button moved about 100 px from under the pointer that pressed it, and
Stop Waiting came there: a second click stopped the wait. The actions of a start whose answer was lost (Open in Patronus,
Forget This Start) came there the same way.

## What Changes

- The actions beside the Safe Push button come on the side away from the edge of its place that stands: on its left in
  the step track of a row and of an island header and in the footer of the preview, on its right among the buttons of
  the robot details, which start their line.

## Capabilities

### Modified Capabilities

- `branch/patronus`: where Stop Waiting of a Safe Push comes, a requirement of its own.

## Impact

- `src/components/branch/SafePushButton.tsx`, `src/components/branch/PatronusChip.tsx`,
  `tests/e2e/patronus/patronus-actions.spec.ts`, `docs/design/ui-theme.md`.
