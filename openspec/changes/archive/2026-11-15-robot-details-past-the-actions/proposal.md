# Proposal

## Why

The card of a robot chip opens before the details of its robot: the session reads its profile, progress, checks and
problems as the card opens, and they come a frame or more later (8 to 23 ms, more under load). They stood above the
actions, so a problem row pushed Safe Push 28 px down from under the pointer (found frame by frame by the probe of
090414076). How many problems come is not known before they come, so their place cannot be held.

## What Changes

- The details that come late (the profile, progress and checks rows, the problems, the line that says they are still to
  come) stand after the actions in a card under its chip, before them in a card over its chip (`data-side` of the popup):
  on the side of the actions away from the edge the card hangs by.
- A test opens the card with the problems held, rests the pointer on Safe Push and lets them come: Safe Push keeps its
  box, 700 and 420 px high, in both engines; on the layout before it moved 28 px down. The open of the card in the
  other tests (`openRobotCard`) no longer only waits for the problems: it checks that Safe Push kept its box as they came.

## Capabilities

### Modified Capabilities

- `branch/branch-row`: the anatomy of the details of a robot, its late details past the actions.

## Impact

- `src/components/branch/PatronusChip.tsx`, `tests/e2e/patronus/patronus-actions.spec.ts`,
  `docs/contributing/frontend.md`.
