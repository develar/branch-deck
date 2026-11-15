# Proposal

## Why

The details of a chip hold still while a press that began in them lasts, and the press ended only with the pointer up
on the window. A press the browser cancels (`pointercancel` without a `pointerup`: a drag of a text or a link of the
details) left them held where they stood, away from the chip, until the next pointer up anywhere (the review of
18769d016). No test pressed in the details and left them with the button down.

## What Changes

- A press of the details ends with the pointer up or with its cancel (`useHeldAnchor`).
- A test presses in the robot card and leaves for the chip with the button down (the card holds, and opens by the chip
  once the press ended), and cancels a press (the card stands by the chip, the button down).

## Capabilities

### Modified Capabilities

- `branch/branch-row`: a press of the details of a chip ends with its cancel too.

## Impact

- `src/components/branch/StatusChip.tsx`, `tests/e2e/patronus/patronus-actions.spec.ts`.
