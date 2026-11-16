# Proposal

## Why

The start line of a running robot stands above the actions but changed with its details: the time left left it when
the estimate of the CI came, and the profile the details named changed what the usual time is of. The line wrapped once
less and the actions moved up about 16 px under the pointer (R3 of 46ece10d6). Every test used a failed robot.

## What Changes

- The start line of a running robot says only what the card knew as it opened: its time and the usual time of the
  profile known then. The time left is in the row Progress, past the actions: from history until the CI tells it.
- A profile known as the card opened stands in the rows above the actions; one the details name later comes past them.
- The details of a chip measure the edge they hang by also as their positioner moves them without a change of size, and
  measure it anew at each open; a press ends also with a pointer up outside the window (its blur); a pinned card passes
  the window without collision avoidance, and a chip rendered anew in the place of another holds no pin of it: accepted.
- E2E: the card of a running robot under its chip, its checks held, the pointer on Cancel: Cancel keeps its box.

## Capabilities

### Modified Capabilities

- `branch/patronus`: the start line and the row Progress of a running robot.
- `branch/branch-row`: the details of a chip hold still without collision avoidance.

## Impact

- `src/components/branch/PatronusChip.tsx`, `src/components/branch/StatusChip.tsx`, `src/lib/robot-eta.ts`,
  `tests/e2e/patronus/`, `docs/contributing/frontend.md`.
