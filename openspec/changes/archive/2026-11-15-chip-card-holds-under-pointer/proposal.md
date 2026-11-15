# Proposal

## Why

The details of a chip of the Status column hang from the chip, which moves as the Actions of the row widen. A Safe Push
pressed in the robot card waited for the repository lock, Stop Waiting came in the slot of its row (the step track
104 to 201.6 px), the chip moved 97 px left, and the card with it: away from the pointer that pressed Safe Push, and at
a window 700 px high the card closed as it left the pointer.

## What Changes

- While the pointer is in the details of a chip, or a press that began in them lasts, they hold still: their anchor is
  the place the chip had as the pointer came (`useHeldAnchor` of `StatusChip`). Once the pointer left them and no press
  lasts, the chip is their anchor again, which places them anew. Base UI 1.9 re-arms the tracking of the anchor only for
  a new anchor, not for a change of `disableAnchorTracking`, so the anchor is what changes.

## Capabilities

### Modified Capabilities

- `branch/branch-row`: the details of a chip hold still under the pointer.

## Impact

- `src/components/branch/StatusChip.tsx`, `tests/e2e/patronus/patronus-actions.spec.ts`, `playwright.webkit.config.ts`,
  `docs/contributing/frontend.md`.
